#![cfg_attr(windows, windows_subsystem = "windows")]

mod crypto;

use actix_web::{web, App, HttpServer, HttpResponse};
use chrono::{Datelike, Local};
use log::{info, warn};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use crate::crypto::{EncryptedData, derive_key, encrypt, decrypt, generate_recovery_key};

/// 密码提示符文件路径（存储密码提示和加密元数据）
fn password_meta_path() -> String {
    res_path("meituan.meta.json")
}

/// 密码元数据
#[derive(Serialize, Deserialize, Clone, Debug)]
struct PasswordMeta {
    /// 密码提示（用户设置的）
    hint: String,
    /// 恢复密钥
    recovery_key: String,
    /// Cookie 文件的加密数据（用于验证密码）
    encrypted_cookies: Option<EncryptedData>,
}

/// 提示用户输入密码（隐藏输入）
fn prompt_password(prompt: &str) -> String {
    println!("\n{}", prompt);
    print!("> ");
    std::io::stdout().flush().unwrap();
    rpassword::read_password().unwrap_or_default()
}

/// 首次设置密码
fn setup_password() -> (String, PasswordMeta) {
    println!("\n{}", "═".repeat(50));
    println!("  🔐 首次使用，请设置主密码");
    println!("{}", "═".repeat(50));
    println!();
    println!("  ⚠️  主密码用于加密您的 Cookie 和订单数据");
    println!("  ⚠️  忘记密码将导致数据永久不可恢复");
    println!("  ⚠️  请使用 12 位以上复杂密码");
    println!();
    
    let password = prompt_password("请输入主密码（≥12 位）:");
    if password.len() < 12 {
        println!("❌ 密码太短，至少需要 12 位");
        std::process::exit(1);
    }
    
    let confirm = prompt_password("请再次输入主密码确认:");
    if password != confirm {
        println!("❌ 两次输入的密码不一致");
        std::process::exit(1);
    }
    
    println!();
    print!("请输入密码提示（可选，用于提醒自己）: ");
    std::io::stdout().flush().unwrap();
    let mut hint = String::new();
    std::io::stdin().read_line(&mut hint).unwrap();
    let hint = hint.trim().to_string();
    
    let recovery_key = generate_recovery_key();
    
    println!();
    println!("  🔑 恢复密钥（请妥善保存）:");
    println!("  ┌──────────────────────────────────────────┐");
    println!("  │  {:<40} │", recovery_key);
    println!("  └──────────────────────────────────────────┘");
    println!();
    println!("  ⚠️  恢复密钥用于忘记主密码时重置");
    println!("  ⚠️  请打印或保存到密码管理器");
    println!("  ⚠️  丢失密钥+忘记密码 = 数据永久丢失");
    println!();
    println!("按 Enter 继续...");
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf).unwrap();
    
    let meta = PasswordMeta {
        hint,
        recovery_key,
        encrypted_cookies: None,
    };
    
    // 保存元数据
    let meta_json = serde_json::to_string_pretty(&meta).unwrap();
    std::fs::write(password_meta_path(), meta_json).unwrap();
    
    (password, meta)
}

/// 验证密码并获取密钥
fn verify_password(meta: &PasswordMeta) -> [u8; 32] {
    let prompt = if meta.hint.is_empty() {
        "请输入主密码:".to_string()
    } else {
        format!("请输入主密码（提示: {}）:", meta.hint)
    };
    let password = prompt_password(&prompt);
    
    if let Some(encrypted) = &meta.encrypted_cookies {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let salt = STANDARD.decode(&encrypted.salt).unwrap();
        let key = derive_key(&password, &salt);
        if decrypt(encrypted, &key).is_ok() {
            return key;
        }
    }
    
    println!("❌ 密码错误");
    std::process::exit(1);
}

/// 加密保存 Cookie 文件
fn save_encrypted_cookies(cookies_json: &str, key: &[u8; 32], meta: &mut PasswordMeta) {
    let encrypted = encrypt(cookies_json.as_bytes(), key);
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    
    // 保存加密后的 Cookie
    let enc_json = serde_json::to_string_pretty(&encrypted).unwrap();
    std::fs::write(res_path("meituan_cookies.enc"), enc_json).unwrap();
    
    // 更新元数据
    meta.encrypted_cookies = Some(encrypted.clone());
    let meta_json = serde_json::to_string_pretty(meta).unwrap();
    std::fs::write(password_meta_path(), meta_json).unwrap();
}

/// 解密 Cookie 文件
fn load_decrypted_cookies(meta: &PasswordMeta, key: &[u8; 32]) -> String {
    let enc_path = res_path("meituan_cookies.enc");
    if !std::path::Path::new(&enc_path).exists() {
        return String::new();
    }
    
    let enc_json = std::fs::read_to_string(&enc_path).unwrap();
    let encrypted: EncryptedData = serde_json::from_str(&enc_json).unwrap();
    let decrypted = decrypt(&encrypted, key).unwrap();
    String::from_utf8(decrypted).unwrap()
}
use std::time::Instant;

// This will be removed - the frontend now doesn't call query() on init
// The frontend version is in meituan_query.html

// ═══════════════════════════════════════════════════════════════════
//  日志系统：输出到文件 meituan-rs.log
// ═══════════════════════════════════════════════════════════════════

fn init_logging(log_dir: &str) {
    let log_path = format!("{}/meituan-rs.log", log_dir);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .unwrap_or_else(|e| panic!("无法创建日志文件 {}: {}", log_path, e));
    
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .filter(Some("actix_server"), log::LevelFilter::Error)
        .filter(Some("actix_web"), log::LevelFilter::Error)
        .target(env_logger::Target::Pipe(Box::new(file)))
        .format(|buf, record| {
            use std::io::Write;
            let now = chrono::Local::now();
            let level = match record.level() {
                log::Level::Error => "❌",
                log::Level::Warn => "⚠️",
                log::Level::Info => "✅",
                log::Level::Debug => "🔍",
                log::Level::Trace => "📝",
            };
            writeln!(buf, "{} [{}] {}",
                now.format("%Y-%m-%d %H:%M:%S"),
                level,
                record.args()
            )
        })
        .init();
}

// ═══════════════════════════════════════════════════════════════════
//  Models
// ═══════════════════════════════════════════════════════════════════

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Order {
    pub coupon_value: Option<String>,
    pub product_info: Option<String>,
    pub product_type: Option<String>,
    pub sale_price: Option<String>,
    pub discount_price: Option<String>,
    pub consume_date: Option<String>,
    pub mobile: Option<String>,
    pub description: Option<String>,
    pub shop_info: Option<String>,
    pub is_refunded: bool,
}

#[derive(Serialize, Debug)]
pub struct ProductStat {
    pub name: String,
    pub count: i64,
}

#[derive(Serialize, Debug)]
pub struct MonthlyStat {
    pub month: String,
    pub count: i64,
    pub fee_total: f64,
}

#[derive(Serialize, Debug)]
pub struct StatsResponse {
    pub total: i64,
    pub refunded: i64,
    pub min_date: Option<String>,
    pub max_date: Option<String>,
    pub products: Vec<ProductStat>,
    pub monthly: Vec<MonthlyStat>,
    pub shifts: HashMap<String, i64>,
    pub build_version: String,
}

/// 从编译时注入的 BUILD_TIME 环境变量获取版本号
fn get_build_version() -> String {
    format!("v{}", env!("BUILD_TIME"))
}
/// 按计费规则计算订单计费价(读取 settings.json 的 fee_json, 无配置则用默认规则)
struct FeePlan {
    cat: String,
    plan: String,
    fee: f64,
}

fn load_fee_plans(exe_dir: &str) -> Vec<FeePlan> {
    let sp = format!("{}/settings.json", exe_dir);
    if let Ok(s) = std::fs::read_to_string(&sp) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
            if let Some(fj) = v.get("fee_json").and_then(|x| x.as_str()) {
                if let Ok(plans) = serde_json::from_str::<Vec<serde_json::Value>>(fj) {
                    return plans.into_iter().filter_map(|p| {
                        let cat = p.get("cat").and_then(|x| x.as_str())?.to_string();
                        let plan = p.get("plan").and_then(|x| x.as_str())?.to_string();
                        let fee = p.get("fee").and_then(|x| x.as_f64())?;
                        Some(FeePlan { cat, plan, fee })
                    }).collect();
                }
            }
        }
    }
    vec![]
}

fn calc_fee(plans: &[FeePlan], pi: &str) -> f64 {
    for p in plans {
        if !p.cat.is_empty() && !p.plan.is_empty() && pi.contains(&p.cat) && pi.contains(&p.plan) {
            return p.fee;
        }
    }
    // 默认规则(与前端 getFeePlans 一致)
    if pi.contains("包天") && !pi.contains("普通区") && !pi.contains("骑手") { return 100.0; }
    if pi.contains("电竞") && pi.contains("包天") { return 110.0; }
    if pi.contains("包夜") && pi.contains("电竞") { return 55.0; }
    if pi.contains("通宵") { return 78.0; }
    if pi.contains("包夜") && pi.contains("网游") { return 45.0; }
    if pi.contains("包夜") && pi.contains("普通") { return 30.0; }
    if pi.contains("包夜") { return 40.0; }
    if pi.contains("包早") { return 25.0; }
    if pi.contains("包天") && pi.contains("普通") { return 70.0; }
    if pi.contains("包天") && pi.contains("网游") { return 90.0; }
    if pi.contains("5070") && pi.contains("4小时") { return 44.0; }
    if pi.contains("5070") && pi.contains("3小时") { return 34.0; }
    if pi.contains("4小时") && pi.contains("网游") { return 36.0; }
    if pi.contains("4小时") { return 40.0; }
    if pi.contains("3小时") && pi.contains("网游") { return 26.0; }
    if pi.contains("3小时") { return 30.0; }
    if pi.contains("新会员") { return 30.0; }
    if pi.contains("生日") { return 66.0; }
    if pi.contains("会员卡") || pi.contains("网费") || pi.contains("送") {
        if pi.contains("充1000") { return 30.0; }
        if pi.contains("1000") { return 1000.0; }
        return 100.0;
    }
    30.0 // 默认
}


#[derive(Serialize, Debug)]
pub struct QueryResponse {
    pub total: i64,
    pub rows: Vec<Order>,
}

#[derive(Serialize, Debug)]
pub struct DetailProduct {
    pub name: String,
    pub total: i64,
    pub refunded: i64,
    pub revenue: f64,
    pub day_shift: i64,
    pub night_shift: i64,
}

#[derive(Serialize, Debug)]
pub struct DetailResponse {
    pub products: Vec<DetailProduct>,
    pub trends: HashMap<String, HashMap<String, i64>>,
}

#[derive(Serialize, Debug)]
pub struct HealthResponse {
    pub status: String,
    pub total_rows: i64,
    pub min_db_date: Option<String>,
    pub max_db_date: Option<String>,
    pub cookie_status: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RefreshResponse {
    pub new: i64,
    pub updated: i64,
    pub time: String,
    pub errors: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ColumnSetting {
    pub id: String,
    pub vis: bool,
    pub copy: bool,
}

impl Default for ColumnSetting {
    fn default() -> Self {
        Self { id: String::new(), vis: true, copy: true }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ShiftSetting {
    pub day_start: Vec<i32>,   // [8, 0]
    pub day_end: Vec<i32>,     // [20, 0]
    pub night_start: Vec<i32>, // [20, 0]
    pub night_end: Vec<i32>,   // [8, 0]
}

impl Default for ShiftSetting {
    fn default() -> Self {
        Self {
            day_start: vec![8, 0],
            day_end: vec![20, 0],
            night_start: vec![20, 0],
            night_end: vec![8, 0],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    #[serde(skip)]
    pub key_info: bool,
    #[serde(skip)]
    pub columns: Vec<ColumnSetting>,
    pub shift: ShiftSetting,
    pub fee_json: String,
    #[serde(skip)]
    pub copy_header: bool,
    pub month_start_cal: bool,
    pub month_end_prev: bool,
    pub auto_login: bool,
    pub cookie_raw: String,
    pub refresh_interval_secs: i64,
    pub auto_open_browser: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let default_fee = serde_json::json!([
            {"cat":"新会员","plan":"特惠","fee":30},{"cat":"新会员","plan":"女神","fee":30},{"cat":"新会员","plan":"超值","fee":100},
            {"cat":"5070显卡","plan":"3小时","fee":34},{"cat":"5070显卡","plan":"4小时","fee":44},{"cat":"5070显卡","plan":"包天","fee":110},
            {"cat":"网游区","plan":"3小时","fee":26},{"cat":"网游区","plan":"4小时","fee":36},{"cat":"网游区","plan":"包天","fee":90},
            {"cat":"网游区","plan":"包早","fee":25},{"cat":"网游区","plan":"包夜","fee":45},
            {"cat":"普通区","plan":"包夜","fee":30},{"cat":"普通区","plan":"包天","fee":70},
            {"cat":"老会员","plan":"生日","fee":66},{"cat":"电竞区5070","plan":"通宵","fee":55},
            {"cat":"1000网费","plan":"送500","fee":1000},{"cat":"100网费","plan":"送20","fee":100}
        ]);
        Self {
            key_info: false,
            columns: vec![
                ColumnSetting{id:"product_info".into(),vis:true,copy:true},
                ColumnSetting{id:"product_type".into(),vis:true,copy:true},
                ColumnSetting{id:"coupon_value".into(),vis:true,copy:true},
                ColumnSetting{id:"sale_price".into(),vis:true,copy:true},
                ColumnSetting{id:"discount_price".into(),vis:true,copy:true},
                ColumnSetting{id:"financial".into(),vis:false,copy:false},
                ColumnSetting{id:"consume_date".into(),vis:true,copy:true},
                ColumnSetting{id:"mobile".into(),vis:true,copy:true},
                ColumnSetting{id:"description".into(),vis:true,copy:true},
                ColumnSetting{id:"shop_info".into(),vis:true,copy:true},
                ColumnSetting{id:"fee".into(),vis:true,copy:false},
            ],
            shift: ShiftSetting{
                day_start:vec![8,0], day_end:vec![20,0],
                night_start:vec![20,0], night_end:vec![8,0],
            },
            fee_json: default_fee.to_string(),
            copy_header: false,
            month_start_cal: false,
            month_end_prev: false,
            auto_login: true,
            cookie_raw: "".into(),
            refresh_interval_secs: 60,
            auto_open_browser: false,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
//  App State
// ═══════════════════════════════════════════════════════════════════

pub struct AppState {
    pub db: Pool<SqliteConnectionManager>,
    pub cookie_file: String,
    pub html_dir: String,
    pub exe_dir: String,
    /// 加密密钥（运行时从密码派生）
    pub enc_key: [u8; 32],
}

// ═══════════════════════════════════════════════════════════════════
//  Database
// ═══════════════════════════════════════════════════════════════════

fn init_db(pool: &Pool<SqliteConnectionManager>) -> Result<(), Box<dyn std::error::Error>> {
    let conn = pool.get()?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS orders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            coupon_value TEXT UNIQUE,
            order_id TEXT,
            product_info TEXT,
            product_type TEXT,
            sale_price TEXT,
            discount_price TEXT,
            consume_date TEXT,
            mobile TEXT,
            description TEXT,
            shop_info TEXT,
            verify_account TEXT,
            is_refunded INTEGER DEFAULT 0,
            extra_json TEXT,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_consume_date ON orders(consume_date);
        CREATE INDEX IF NOT EXISTS idx_coupon_value ON orders(coupon_value);
        CREATE INDEX IF NOT EXISTS idx_is_refunded ON orders(is_refunded);"
    )?;
    info!("数据库初始化完成（SQLCipher 加密）");
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
//  Handlers
// ═══════════════════════════════════════════════════════════════════

fn json_ok<T: Serialize>(data: T) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header(("X-Content-Type-Options", "nosniff"))
        .insert_header(("Cache-Control", "no-store"))
        .content_type("application/json; charset=utf-8")
        .json(data)
}

fn json_err(e: String) -> HttpResponse {
    HttpResponse::InternalServerError()
        .insert_header(("X-Content-Type-Options", "nosniff"))
        .insert_header(("Cache-Control", "no-store"))
        .content_type("application/json; charset=utf-8")
        .json(serde_json::json!({"error": e}))
}

async fn handle_index(state: web::Data<AppState>) -> HttpResponse {
    let path = format!("{}/meituan_query.html", state.html_dir);
    match fs::read_to_string(&path) {
        Ok(html) => HttpResponse::Ok()
            .insert_header(("X-Content-Type-Options", "nosniff"))
            .insert_header(("Cache-Control", "no-store, must-revalidate"))
            .insert_header(("Pragma", "no-cache"))
            .insert_header(("Expires", "0"))
            .content_type("text/html; charset=utf-8")
            .body(html),
        Err(e) => HttpResponse::NotFound().insert_header(("X-Content-Type-Options", "nosniff")).body(format!("Page not found: {}", e)),
    }
}

async fn handle_settings_page(state: web::Data<AppState>) -> HttpResponse {
    let path = format!("{}/meituan_settings.html", state.html_dir);
    match fs::read_to_string(&path) {
        Ok(html) => HttpResponse::Ok()
            .insert_header(("X-Content-Type-Options", "nosniff"))
            .insert_header(("Cache-Control", "no-store, must-revalidate"))
            .content_type("text/html; charset=utf-8")
            .body(html),
        Err(e) => HttpResponse::NotFound().insert_header(("X-Content-Type-Options", "nosniff")).body(format!("Settings page not found: {}", e)),
    }
}



async fn handle_logo(state: web::Data<AppState>) -> HttpResponse {
    let path = format!("{}/logo.png", state.html_dir);
    match std::fs::read(&path) {
        Ok(bytes) => HttpResponse::Ok()
            .content_type("image/png")
            .body(bytes),
        Err(e) => HttpResponse::NotFound().body(format!("Logo not found: {}", e)),
    }
}

async fn handle_health(state: web::Data<AppState>) -> HttpResponse {
    let cookie_ok = std::path::Path::new(&enc_cookie_path()).exists();
    match state.db.get() {
        Ok(conn) => {
            let total = db_total(&conn);
            let (mn, mx) = db_daterange(&conn);
            json_ok(HealthResponse {
                status: "ok".into(),
                total_rows: total,
                min_db_date: mn,
                max_db_date: mx,
                cookie_status: if cookie_ok { "ok".into() } else { "missing".into() },
            })
        }
        Err(e) => json_err(format!("{}", e)),
    }
}

fn db_total(conn: &rusqlite::Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM orders", [], |r| r.get(0)).unwrap_or(0)
}
fn db_refunded(conn: &rusqlite::Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM orders WHERE is_refunded=1", [], |r| r.get(0)).unwrap_or(0)
}
fn db_daterange(conn: &rusqlite::Connection) -> (Option<String>, Option<String>) {
    conn.query_row("SELECT MIN(consume_date), MAX(consume_date) FROM orders", [],
        |r| Ok((r.get(0)?, r.get(1)?))).unwrap_or((None, None))
}

async fn handle_stats(state: web::Data<AppState>) -> HttpResponse {
    match state.db.get() {
        Ok(conn) => {
            let total = db_total(&conn);
            let refunded = db_refunded(&conn);
            let (mn, mx) = db_daterange(&conn);

            let products = match conn.prepare(
                "SELECT product_info, COUNT(*) FROM orders GROUP BY product_info ORDER BY COUNT(*) DESC"
            ) {
                Ok(mut stmt) => stmt.query_map([], |r| Ok(ProductStat { name: r.get(0)?, count: r.get(1)? }))
                    .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<ProductStat>>())
                    .unwrap_or_default(),
                Err(_) => vec![],
            };

            // 月度统计：订单数 + 计费价合计
            let plans = load_fee_plans(&state.exe_dir);
            let monthly_data = match conn.prepare(
                "SELECT strftime('%Y-%m', datetime(consume_date, '-8 hours')) as m, product_info FROM orders WHERE product_info IS NOT NULL AND product_info != '' AND is_refunded = 0"
            ) {
                Ok(mut stmt) => {
                    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                        .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<(String, String)>>())
                        .unwrap_or_default();
                    let mut map: HashMap<String, (i64, f64)> = HashMap::new();
                    for (month, pi) in rows {
                        let fee = calc_fee(&plans, &pi);
                        let entry = map.entry(month).or_insert((0, 0.0));
                        entry.0 += 1;
                        entry.1 += fee;
                    }
                    let mut v: Vec<MonthlyStat> = map.into_iter()
                        .map(|(m, (c, f))| MonthlyStat { month: m, count: c, fee_total: f })
                        .collect();
                    v.sort_by(|a, b| a.month.cmp(&b.month));
                    v
                }
                Err(_) => vec![],
            };

            let shifts = match conn.prepare(
                "SELECT CASE WHEN CAST(strftime('%H',consume_date) AS INTEGER)>=8 \
                 AND CAST(strftime('%H',consume_date) AS INTEGER)<20 THEN 'day' ELSE 'night' END as s, \
                 COUNT(*) FROM orders GROUP BY s"
            ) {
                Ok(mut stmt) => stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
                    .map(|rows| rows.filter_map(|r| r.ok()).collect::<HashMap<String, i64>>())
                    .unwrap_or_default(),
                Err(_) => HashMap::new(),
            };

	            json_ok(StatsResponse { total, refunded, min_date: mn, max_date: mx, products, monthly: monthly_data, shifts, build_version: get_build_version() })
        }
        Err(e) => json_err(format!("{}", e)),
    }
}

async fn handle_query(
    state: web::Data<AppState>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    match state.db.get() {
        Ok(conn) => {
            let mut cond: Vec<String> = Vec::new();
            let mut vals: Vec<String> = Vec::new();

            if let Some(s) = query.get("beginDate") {
                cond.push("consume_date >= ?".into());
                vals.push(s.clone());
            }
            if let Some(e) = query.get("endDate") {
                cond.push("consume_date <= ?".into());
                vals.push(e.clone());
            }
            if let Some(p) = query.get("productInfo") {
                cond.push("product_info LIKE ?".into());
                vals.push(format!("%{}%", p));
            }
            if let Some(c) = query.get("couponValue") {
                cond.push("coupon_value LIKE ?".into());
                vals.push(format!("%{}%", c));
            }
            if let Some(m) = query.get("mobile") {
                cond.push("mobile LIKE ?".into());
                vals.push(format!("%{}%", m));
            }
            if let Some(r) = query.get("isRefunded") {
                if r == "0" || r == "1" {
                    cond.push(format!("is_refunded={}", r));
                }
            }
            let limit: i64 = query.get("limit").and_then(|v| v.parse().ok()).unwrap_or(2000);
            let offset: i64 = query.get("offset").and_then(|v| v.parse().ok()).unwrap_or(0);

            let where_sql = if cond.is_empty() { "1=1".into() } else { cond.join(" AND ") };
            let total: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM orders WHERE {}", where_sql),
                    rusqlite::params_from_iter(vals.iter()), |r| r.get(0))
                .unwrap_or(0);

            let sql = format!(
                "SELECT coupon_value,product_info,product_type,sale_price, \
                 discount_price,consume_date,mobile,description,shop_info,is_refunded \
                 FROM orders WHERE {} ORDER BY consume_date DESC LIMIT ? OFFSET ?",
                where_sql
            );
            let mut all_vals = vals.clone();
            all_vals.push(limit.to_string());
            all_vals.push(offset.to_string());

            let rows = match conn.prepare(&sql) {
                Ok(mut stmt) => stmt
                    .query_map(rusqlite::params_from_iter(all_vals.iter()), |r| {
                        Ok(Order {
                            coupon_value: r.get::<_, Option<String>>(0).unwrap_or(None),
                            product_info: r.get::<_, Option<String>>(1).unwrap_or(None),
                            product_type: r.get::<_, Option<String>>(2).unwrap_or(None),
                            sale_price: r.get::<_, Option<String>>(3).unwrap_or(None),
                            discount_price: r.get::<_, Option<String>>(4).unwrap_or(None),
                            consume_date: r.get::<_, Option<String>>(5).unwrap_or(None),
                            mobile: r.get::<_, Option<String>>(6).unwrap_or(None),
                            description: r.get::<_, Option<String>>(7).unwrap_or(None),
                            shop_info: r.get::<_, Option<String>>(8).unwrap_or(None),
                            is_refunded: r.get::<_, i64>(9).unwrap_or(0) != 0,
                        })
                    })
                    .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<Order>>())
                    .unwrap_or_default(),
                Err(_) => vec![],
            };

            json_ok(QueryResponse { total, rows })
        }
        Err(e) => json_err(format!("{}", e)),
    }
}

async fn handle_stats_detail(state: web::Data<AppState>, query: web::Query<HashMap<String, String>>) -> HttpResponse {
    match state.db.get() {
        Ok(conn) => {
            // 确定统计周期: monthly/quarterly/semi/annual
            let period = query.get("period").map(|s| s.as_str()).unwrap_or("monthly");
            // 确定年度起始年，默认当前年
            let year: i32 = query.get("year").and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                chrono::Local::now().year()
            });
            let year_start = format!("{}-01-01 08:00:00", year);
            let year_end = format!("{}-01-01 08:00:00", year + 1);

            // 构建周期列表
            let periods = build_periods(year, period);

            let mut result_periods: Vec<serde_json::Value> = Vec::new();
            let mut year_total = 0i64;
            let mut year_fee = 0.0;
            let mut year_day_count = 0i64;
            let mut year_night_count = 0i64;
            let mut year_day_fee = 0.0;
            let mut year_night_fee = 0.0;

            let plans = load_fee_plans(&state.exe_dir);
            for (label, p_start, p_end) in &periods {
                // 统计该周期
                let sql = "SELECT COUNT(*), \
                    SUM(CASE WHEN CAST(strftime('%H',consume_date) AS INTEGER)>=8 \
                        AND CAST(strftime('%H',consume_date) AS INTEGER)<20 THEN 1 ELSE 0 END), \
                    SUM(CASE WHEN NOT(CAST(strftime('%H',consume_date) AS INTEGER)>=8 \
                        AND CAST(strftime('%H',consume_date) AS INTEGER)<20) THEN 1 ELSE 0 END), \
                    product_info \
                    FROM orders WHERE consume_date>=? AND consume_date<=? AND is_refunded=0 GROUP BY product_info";
                let mut stmt = match conn.prepare(sql) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let rows = stmt.query_map(rusqlite::params![p_start, p_end], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?,
                        r.get::<_, String>(3)?))
                }).ok().map(|r| r.filter_map(|x| x.ok()).collect::<Vec<_>>()).unwrap_or_default();

                let mut p_total = 0i64;
                let mut p_day_count = 0i64;
                let mut p_night_count = 0i64;
                let mut p_fee = 0.0;
                let mut p_day_fee = 0.0;
                let mut p_night_fee = 0.0;

                for (total, day, night, pi) in rows {
                    let fee = calc_fee(&plans, &pi);
                    p_total += total;
                    p_day_count += day;
                    p_night_count += night;
                    p_fee += fee * (total as f64);
                    p_day_fee += fee * (day as f64);
                    p_night_fee += fee * (night as f64);
                }

                year_total += p_total;
                year_fee += p_fee;
                year_day_count += p_day_count;
                year_night_count += p_night_count;
                year_day_fee += p_day_fee;
                year_night_fee += p_night_fee;

                let mut period_obj = serde_json::json!({
                    "label": label,
                    "start": p_start,
                    "end": p_end,
                    "total": p_total,
                    "fee_total": (p_fee * 100.0).round() / 100.0,
                    "day": {"count": p_day_count, "fee": (p_day_fee * 100.0).round() / 100.0},
                    "night": {"count": p_night_count, "fee": (p_night_fee * 100.0).round() / 100.0},
                });

                // 月度视图：列出每天明细
                if period == "monthly" {
                    let daily = build_daily_breakdown(&conn, &state.exe_dir, p_start, p_end);
                    period_obj["days"] = serde_json::json!(daily);
                }

                result_periods.push(period_obj);
            }

            let resp = serde_json::json!({
                "period": period,
                "year": year,
                "year_start": year_start,
                "year_end": year_end,
                "summary": {
                    "total": year_total,
                    "fee_total": (year_fee * 100.0).round() / 100.0,
                    "day": {"count": year_day_count, "fee": (year_day_fee * 100.0).round() / 100.0},
                    "night": {"count": year_night_count, "fee": (year_night_fee * 100.0).round() / 100.0},
                },
                "periods": result_periods,
            });
            json_ok(resp)
        }
        Err(e) => json_err(format!("{}", e)),
    }
}

/// 构建周期列表
fn build_periods(year: i32, period: &str) -> Vec<(String, String, String)> {
    match period {
        "quarterly" => {
            vec![
                ("Q1".into(), format!("{}-01-01 08:00:00", year), format!("{}-04-01 08:00:00", year)),
                ("Q2".into(), format!("{}-04-01 08:00:00", year), format!("{}-07-01 08:00:00", year)),
                ("Q3".into(), format!("{}-07-01 08:00:00", year), format!("{}-10-01 08:00:00", year)),
                ("Q4".into(), format!("{}-10-01 08:00:00", year), format!("{}-01-01 08:00:00", year + 1)),
            ]
        }
        "semi" => {
            vec![
                ("上半年".into(), format!("{}-01-01 08:00:00", year), format!("{}-07-01 08:00:00", year)),
                ("下半年".into(), format!("{}-07-01 08:00:00", year), format!("{}-01-01 08:00:00", year + 1)),
            ]
        }
        "annual" => {
            vec![
                ("全年".into(), format!("{}-01-01 08:00:00", year), format!("{}-01-01 08:00:00", year + 1)),
            ]
        }
        _ => { // monthly
            let mut v = Vec::new();
            for m in 1..=12 {
                let start = format!("{:04}-{:02}-01 08:00:00", year, m);
                let end = if m == 12 {
                    format!("{:04}-01-01 08:00:00", year + 1)
                } else {
                    format!("{:04}-{:02}-01 08:00:00", year, m + 1)
                };
                v.push((format!("{}月", m), start, end));
            }
            v
        }
    }
}

/// 月度视图：每天白班/夜班明细
fn build_daily_breakdown(conn: &rusqlite::Connection, exe_dir: &str, start: &str, end: &str) -> Vec<serde_json::Value> {
    let plans = load_fee_plans(exe_dir);
    let sql = "SELECT consume_date, product_info FROM orders WHERE consume_date>=? AND consume_date<=? AND is_refunded=0";
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let rows = stmt.query_map(rusqlite::params![start, end], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    }).ok().map(|r| r.filter_map(|x| x.ok()).collect::<Vec<_>>()).unwrap_or_default();

    // Key: "MM-DD", Value: (day_count, day_fee, night_count, night_fee)
    let mut daily_map: std::collections::HashMap<String, (i64, f64, i64, f64)> = std::collections::HashMap::new();

    for (cdate, pi) in rows {
        if cdate.len() < 10 { continue; }
        let date_key = cdate[5..10].to_string(); // 提取 MM-DD
        let fee = calc_fee(&plans, &pi);

        // 判断班次（根据小时是否在8~20点之间）
        let is_day_shift = if let Some(hour_str) = cdate.split(' ').nth(1).and_then(|t| t.split(':').next()) {
            if let Ok(h) = hour_str.parse::<i32>() {
                h >= 8 && h < 20
            } else {
                true
            }
        } else {
            true
        };

        let entry = daily_map.entry(date_key).or_insert((0, 0.0, 0, 0.0));
        if is_day_shift {
            entry.0 += 1;
            entry.1 += fee;
        } else {
            entry.2 += 1;
            entry.3 += fee;
        }
    }

    let mut sorted_keys: Vec<String> = daily_map.keys().cloned().collect();
    sorted_keys.sort();

    sorted_keys.iter().map(|k| {
        let &(day, day_fee, night, night_fee) = daily_map.get(k).unwrap();
        serde_json::json!({
            "date": k,
            "day": {"count": day, "fee": (day_fee * 100.0).round() / 100.0},
            "night": {"count": night, "fee": (night_fee * 100.0).round() / 100.0},
        })
    }).collect()
}

// ═══════════════════════════════════════════════════════════════════
//  数据刷新（调用Python脚本，requests库更稳定）
// ═══════════════════════════════════════════════════════════════════

static REFRESH_LOCK: AtomicBool = AtomicBool::new(false);
static REFRESH_LOCK_TIME: Mutex<Option<Instant>> = Mutex::new(None);
const SYNC_TIMEOUT: Duration = Duration::from_secs(3600); // 1小时超时自动释放（首次同步需要较长时间）

fn try_acquire_sync_lock() -> bool {
    // 检查是否已被占用
    if REFRESH_LOCK.load(std::sync::atomic::Ordering::Relaxed) {
        // 检查是否超时
        if let Ok(guard) = REFRESH_LOCK_TIME.lock() {
            if let Some(start) = *guard {
                if start.elapsed() > SYNC_TIMEOUT {
                    // 超时，强制释放
                    REFRESH_LOCK.store(false, std::sync::atomic::Ordering::Relaxed);
                    info!("同步锁超时 {} 秒，强制释放", SYNC_TIMEOUT.as_secs());
                }
            }
        } else {
            return false;
        }
    }
    // 尝试获取锁
    if REFRESH_LOCK.compare_exchange(false, true, std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::Relaxed).is_ok() {
        if let Ok(mut guard) = REFRESH_LOCK_TIME.lock() {
            *guard = Some(Instant::now());
        }
        true
    } else {
        false
    }
}

fn release_sync_lock() {
    REFRESH_LOCK.store(false, std::sync::atomic::Ordering::Relaxed);
    if let Ok(mut guard) = REFRESH_LOCK_TIME.lock() {
        *guard = None;
    }
}

fn rust_refresh(exe_dir: &str, deep: bool) -> RefreshResponse {
    if !try_acquire_sync_lock() {
        return RefreshResponse {
            new: 0,
            updated: 0,
            time: Local::now().format("%H:%M:%S").to_string(),
            errors: vec!["已有同步任务正在运行，请稍后再试".into()],
        };
    }
    struct SyncGuard;
    impl Drop for SyncGuard {
        fn drop(&mut self) {
            release_sync_lock();
        }
    }
    let _guard = SyncGuard;
    let sync_start = Instant::now();
    println!("  🔄 同步开始 ({})...", if deep { "深度" } else { "快速" });
    let now = Local::now().format("%H:%M:%S").to_string();
    let cookie_file = format!("{}/meituan_cookies.json", exe_dir);
    let db_path = format!("{}/meituan_orders.db", exe_dir);
    let py_script = format!("{}/meituan_sync.py", exe_dir);

    // 获取最新时间
    let conn = match rusqlite::Connection::open(&db_path) {
        Ok(c) => c,
        Err(e) => return RefreshResponse { new: 0, updated: 0, time: now, errors: vec![format!("DB: {}", e)] },
    };
    let latest: String = conn.query_row(
        "SELECT COALESCE(MAX(consume_date),'2026-01-01 00:00:00') FROM orders", [],
        |r| r.get(0),
    ).unwrap_or_else(|_| "2026-01-01 00:00:00".into());
    let _ = conn.close();

    let start_ts = chrono::NaiveDateTime::parse_from_str(&latest, "%Y-%m-%d %H:%M:%S")
        .map(|dt| {
            use chrono::TimeZone;
            chrono::FixedOffset::east_opt(8 * 3600)
                .and_then(|offset| offset.from_local_datetime(&dt).earliest())
                .map(|t| t.timestamp_millis())
                .unwrap_or(0)
        }).unwrap_or(0);

    let lookback_ms = if deep { 50 * 3600 * 1000 } else { 15 * 60 * 1000 };
    let end_ts = chrono::Utc::now().timestamp_millis();
    // 首次同步（数据库为空）从 2026-01-01 开始完整拉取
    // 后续同步使用滑动窗口：往前推 lookback_ms，但不超过 48 小时（撤销时效）
    let start_ts = if latest == "2026-01-01 00:00:00" {
        // 数据库为空，从 2026-01-01 开始
        start_ts
    } else {
        // 已有订单，使用滑动窗口
        let min_start_ts = end_ts - lookback_ms;
        if start_ts < min_start_ts {
            min_start_ts
        } else {
            start_ts.saturating_sub(3600 * 1000)
        }
    };
    let api_url = "https://e.dianping.com/couponrecord/queryCouponRecordDetails?yodaReady=h5&csecplatform=4&csecversion=4.2.4";

    // 调用 Python 脚本执行同步
    let output = match std::process::Command::new("python3")
        .arg(&py_script)
        .arg(&cookie_file)
        .arg(&db_path)
        .arg(api_url)
        .arg(start_ts.to_string())
        .arg(end_ts.to_string())
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            return RefreshResponse { new: 0, updated: 0, time: now, errors: vec![format!("执行同步脚本失败: {}", e)] };
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        return RefreshResponse { new: 0, updated: 0, time: now, errors: vec![format!("同步脚本出错: {}", stderr)] };
    }

    // 解析 JSON 结果
    match serde_json::from_str::<serde_json::Value>(&stdout) {
        Ok(result) => {
            if let Some(error) = result.get("error") {
                return RefreshResponse { new: 0, updated: 0, time: now, errors: vec![format!("同步失败: {}", error)] };
            }
            let new_count = result.get("new").and_then(|v| v.as_i64()).unwrap_or(0);
            let updated = result.get("updated").and_then(|v| v.as_i64()).unwrap_or(0);
            let errors: Vec<String> = result.get("errors")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();

            let elapsed = sync_start.elapsed().as_secs();
            if errors.is_empty() {
                println!("  ✅ 同步完成，耗时 {} 秒 (新增 {}, 更新 {})", elapsed, new_count, updated);
            } else {
                println!("  ❌ 同步出错，耗时 {} 秒: {:?}", elapsed, errors);
            }

            RefreshResponse { new: new_count, updated, time: now, errors }
        }
        Err(e) => {
            RefreshResponse { new: 0, updated: 0, time: now, errors: vec![format!("解析同步结果失败: {} | {}", e, stdout)] }
        }
    }
}

async fn handle_refresh(
    state: web::Data<AppState>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let deep = query.get("deep").map(|v| v == "1" || v == "true").unwrap_or(false);
    let exe_dir = state.exe_dir.clone();
    let result = tokio::task::spawn_blocking(move || rust_refresh(&exe_dir, deep))
        .await
        .unwrap_or_else(|e| RefreshResponse {
            new: 0,
            updated: 0,
            time: Local::now().format("%H:%M:%S").to_string(),
            errors: vec![format!("刷新任务异常: {}", e)],
        });
    json_ok(result)
}

// ═══════════════════════════════════════════════════════════════════
//  设置文件读写（全局，多人共享）
// ═══════════════════════════════════════════════════════════════════

fn load_settings() -> Settings {
    let p = res_path("settings.json");
    let mut s = if let Ok(content) = std::fs::read_to_string(&p) {
        serde_json::from_str::<Settings>(&content).unwrap_or_default()
    } else {
        Settings::default()
    };

    normalize_settings(&mut s);
    // 安全要求：GET /api/settings 只返回状态配置，绝不回传 meituan_cookies.json 原文。
    s.cookie_raw.clear();
    s
}

fn normalize_settings(s: &mut Settings) {
    let defaults = ShiftSetting::default();
    if s.shift.day_start.len() != 2 { s.shift.day_start = defaults.day_start.clone(); }
    if s.shift.day_end.len() != 2 { s.shift.day_end = defaults.day_end.clone(); }
    if s.shift.night_start.len() != 2 { s.shift.night_start = defaults.night_start.clone(); }
    if s.shift.night_end.len() != 2 { s.shift.night_end = defaults.night_end.clone(); }
    s.refresh_interval_secs = s.refresh_interval_secs.clamp(5, 3600);
}

fn save_settings(s: &Settings, enc_key: &[u8; 32]) -> Result<(), String> {
    // 如果前端更新了 cookie_raw 且非空，加密写回
    if !s.cookie_raw.trim().is_empty() {
        write_encrypted_cookies(&s.cookie_raw, enc_key)?;
    }

    let mut persisted = s.clone();
    normalize_settings(&mut persisted);
    persisted.cookie_raw.clear();

    let p = res_path("settings.json");
    let json = serde_json::to_string_pretty(&persisted).map_err(|e| format!("{}", e))?;
    std::fs::write(&p, json).map_err(|e| format!("{}", e))?;
    Ok(())
}

async fn handle_get_settings(_state: web::Data<AppState>) -> HttpResponse {
    let s = load_settings();
    json_ok(s)
}

async fn handle_put_settings(state: web::Data<AppState>, body: web::Json<Settings>) -> HttpResponse {
    match save_settings(&body.into_inner(), &state.enc_key) {
        Ok(()) => json_ok(serde_json::json!({"ok":true})),
        Err(e) => json_err(e),
    }
}

/// 加密的 Cookie 文件路径
fn enc_cookie_path() -> String {
    res_path("meituan_cookies.enc")
}

/// 读取并解密 Cookie 文件（返回 JSON 字符串）
fn read_encrypted_cookies(key: &[u8; 32]) -> Result<String, String> {
    let path = enc_cookie_path();
    if !std::path::Path::new(&path).exists() {
        return Ok(String::new());
    }
    let enc_json = std::fs::read_to_string(&path).map_err(|e| format!("读取加密 Cookie 失败: {}", e))?;
    let encrypted: crate::crypto::EncryptedData = serde_json::from_str(&enc_json).map_err(|e| format!("解析加密数据失败: {}", e))?;
    let decrypted = crate::crypto::decrypt(&encrypted, key).map_err(|e| format!("解密失败: {}", e))?;
    String::from_utf8(decrypted).map_err(|e| format!("UTF-8 解码失败: {}", e))
}

/// 加密并保存 Cookie 文件
fn write_encrypted_cookies(content: &str, key: &[u8; 32]) -> Result<(), String> {
    let encrypted = crate::crypto::encrypt(content.as_bytes(), key);
    let enc_json = serde_json::to_string_pretty(&encrypted).map_err(|e| format!("序列化失败: {}", e))?;
    std::fs::write(enc_cookie_path(), enc_json).map_err(|e| format!("写入加密 Cookie 失败: {}", e))?;
    // 删除旧的明文文件（如果存在）
    let plain_path = res_path("meituan_cookies.json");
    if std::path::Path::new(&plain_path).exists() {
        let _ = std::fs::remove_file(&plain_path);
    }
    Ok(())
}

/// 校验 Cookie 是否有效：读取加密文件后调用美团 API 测试
fn validate_cookies(enc_key: &[u8; 32]) -> serde_json::Value {
    let content = match read_encrypted_cookies(enc_key) {
        Ok(c) => c,
        Err(e) => return serde_json::json!({"valid": false, "message": e}),
    };
    
    let cookie_str = match serde_json::from_str::<Vec<serde_json::Value>>(&content) {
        Ok(arr) => arr.iter()
            .filter_map(|c| {
                let name = c.get("name")?.as_str()?;
                let value = c.get("value")?.as_str()?;
                Some(format!("{}={}", name, value))
            }).collect::<Vec<_>>().join("; "),
        Err(e) => return serde_json::json!({"valid": false, "message": format!("Cookie JSON 解析失败: {}", e)}),
    };

    if cookie_str.is_empty() {
        return serde_json::json!({"valid": false, "message": "Cookie 为空，请重新粘贴"});
    }

    // 调用美团 API 测试（只拉取一页，最近 1 天）
    let api_url = "https://e.dianping.com/couponrecord/queryCouponRecordDetails?yodaReady=h5&csecplatform=4&csecversion=4.2.4";
    let end_ts = chrono::Utc::now().timestamp_millis();
    let start_ts = end_ts - 24 * 3600 * 1000;
    let payload = serde_json::json!({
        "dealGroupIds":"","bussinessType":0,"shopIds":"0","productTabNum":1,
        "offset": 0, "limit": 10,
        "beginDate": start_ts, "endDate": end_ts,
        "subTabNum": null, "isConsumeMedical": false
    });

    let payload_str = payload.to_string();
    let py_script = format!("{}/http_helper.py", std::env::current_dir().unwrap_or_default().to_string_lossy());
    let tmp_path = format!("{}/validate_tmp.json", std::env::current_dir().unwrap_or_default().to_string_lossy());
    let output = match std::process::Command::new("python3")
        .arg(&py_script)
        .arg(api_url)
        .arg(&cookie_str)
        .arg(&payload_str)
        .arg(&tmp_path)
        .output()
    {
        Ok(o) => o,
        Err(e) => return serde_json::json!({"valid": false, "message": format!("HTTP 请求失败: {}", e)}),
    };
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("401") || stderr.contains("403") {
            return serde_json::json!({"valid": false, "message": "Cookie 已过期（HTTP 401/403），请在浏览器重新登录美团商家后台后重新提取"});
        }
        return serde_json::json!({"valid": false, "message": format!("HTTP 请求失败: {}", stderr.chars().take(100).collect::<String>())});
    }
    
    let body_str = match std::fs::read_to_string(&tmp_path) {
        Ok(s) => s,
        Err(e) => return serde_json::json!({"valid": false, "message": format!("读取响应失败: {}", e)}),
    };
    let _ = std::fs::remove_file(&tmp_path);
    
    let data: serde_json::Value = match serde_json::from_str(&body_str) {
        Ok(d) => d,
        Err(e) => return serde_json::json!({"valid": false, "message": format!("API 响应解析失败: {}", e)}),
    };

    // 检查是否有 data 字段
    if let Some(d) = data.get("data").and_then(|v| v.as_object()) {
        let record_sum = d.get("recordSum").and_then(|v| v.as_i64()).unwrap_or(0);
        serde_json::json!({
            "valid": true,
            "message": format!("Cookie 有效！最近 1 天共有 {} 条核销记录", record_sum),
            "recordSum": record_sum
        })
    } else if let Some(msg) = data.get("msg").and_then(|v| v.as_str()) {
        serde_json::json!({"valid": false, "message": format!("美团返回: {}", msg)})
    } else {
        serde_json::json!({"valid": false, "message": "API 响应格式异常，Cookie 可能已过期"})
    }
}

async fn handle_validate_cookies(state: web::Data<AppState>) -> HttpResponse {
    json_ok(validate_cookies(&state.enc_key))
}


// ═══════════════════════════════════════════════════════════════════
//  Cookie 登录工具
// ═══════════════════════════════════════════════════════════════════

/// 以启动时的工作目录为基准解析相对路径（cookie/db 在项目根目录）
fn res_path(rel: &str) -> String {
    std::env::current_dir()
        .unwrap_or_default()
        .join(rel)
        .to_string_lossy()
        .to_string()
}

const CDP_PORT: &str = "9222";
const MEITUAN_URL: &str = "https://e.dianping.com/app/merchant-platform/543c7d5810bd431?iUrl=Ly9lLmRpYW5waW5nLmNvbS9hcHAvbnAtbWVyLXZvdWNoZXItd2ViLXN0YXRpYy9yZWNvcmRz";

fn cookie_file_path() -> String { res_path("meituan_cookies.json") }

fn ensure_cookies() -> bool {
    // 检查已有cookie
    let cpath = cookie_file_path();
    if std::path::Path::new(&cpath).exists() {
        if let Ok(content) = fs::read_to_string(&cpath) {
            if let Ok(cookies) = serde_json::from_str::<Vec<Value>>(&content) {
                if cookies.len() > 5 {
                    info!("Cookie: {} 个，无需重新登录", cookies.len());
                    return true;
                }
            }
        }
    }

    println!("\n============================================================");
    println!("  首次使用：需要登录美团商家后台");
    println!("============================================================");

    if wait_for_cdp_fast().is_err() {
        // CDP 不存在时主动启动 Edge，而不是直接放弃首次登录
        let edge = find_edge();
        if edge.is_none() {
            println!("  ⚠️  未找到 Edge 浏览器，跳过自动登录");
            return false;
        }

        println!("  🌐 正在打开浏览器...");
        let _browser = Command::new(edge.unwrap())
            .args([
                &format!("--remote-debugging-port={}", CDP_PORT),
                "--no-first-run", "--no-default-browser-check",
                MEITUAN_URL,
            ])
            .spawn();

        std::thread::sleep(Duration::from_secs(3));
        if wait_for_cdp().is_err() {
            warn!("CDP启动超时，无法自动提取Cookie");
            return false;
        }
    } else {
        info!("检测到已有 CDP 浏览器会话，复用登录窗口");
    }

    println!("\n  请在浏览器窗口中手动登录");
    println!("  登录后脚本会自动提取Cookie并启动服务\n");

    // 等待登录
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(300) {
        std::thread::sleep(Duration::from_secs(2));
        if let Ok(resp) = http_get(&format!("http://127.0.0.1:{}/json", CDP_PORT)) {
            if let Ok(pages) = serde_json::from_str::<Vec<Value>>(&resp) {
                let dp = pages.iter().find(|p| {
                    p["url"].as_str().map(|u| u.contains("dianping")).unwrap_or(false)
                });
                if let Some(p) = dp {
                    let url = p["url"].as_str().unwrap_or("");
                    let ws = p["webSocketDebuggerUrl"].as_str().unwrap_or("");
                    if url.contains("dianping") && !url.contains("login") && !url.contains("passport") && !ws.is_empty() {
                        if let Ok(cookies) = get_cookies_via_cdp(ws) {
                            let dp_count = cookies.iter().filter(|c| {
                                c["domain"].as_str().map(|d| d.contains("dianping")).unwrap_or(false)
                            }).count();
                            if dp_count > 5 {
                                let cpath = cookie_file_path();
                                let _ = fs::write(&cpath, serde_json::to_string_pretty(&cookies).unwrap());
                                println!("\n  ✅ 登录成功！已保存 {} 个 cookie", cookies.len());
                                println!("  ✅ 浏览器可以关闭了，服务已启动\n");
                                return true;
                            }
                        }
                    }
                }
            }
        }
        let elapsed = start.elapsed().as_secs();
        if elapsed.is_multiple_of(15) && elapsed > 0 {
            print!("\r  等待登录... ({}s)", elapsed);
            let _ = std::io::stdout().flush();
        }
    }
    println!("\n  ⚠️  登录超时");
    false
}

fn find_edge() -> Option<String> {
    for p in &[
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    ] { if std::path::Path::new(p).exists() { return Some(p.to_string()); } }
    None
}

fn wait_for_cdp() -> Result<(), String> {
    for _ in 0..30 {
        if http_get(&format!("http://127.0.0.1:{}/json/version", CDP_PORT)).is_ok() { return Ok(()); }
        std::thread::sleep(Duration::from_secs(1));
    }
    Err("CDP timeout".into())
}

fn wait_for_cdp_fast() -> Result<(), String> {
    // 5秒快速检测，用于启动时判断是否需要弹出浏览器
    for _ in 0..5 {
        if http_get(&format!("http://127.0.0.1:{}/json/version", CDP_PORT)).is_ok() { return Ok(()); }
        std::thread::sleep(Duration::from_secs(1));
    }
    Err("CDP not available".into())
}

fn get_cookies_via_cdp(ws_url: &str) -> Result<Vec<Value>, String> {
    let (mut ws, _) = tungstenite::connect(ws_url).map_err(|e| format!("WS: {}", e))?;
    let cmd = json!({"id": 1, "method": "Network.getAllCookies", "params": {}});
    ws.send(tungstenite::Message::Text(serde_json::to_string(&cmd).unwrap().into()))
        .map_err(|e| format!("send: {}", e))?;
    let start = std::time::Instant::now();
    loop {
        if start.elapsed() > Duration::from_secs(30) {
            return Err("CDP cookie获取超时(30s)".into());
        }
        let msg = match ws.read() {
            Ok(m) => m,
            Err(_) => continue, // Timeout, retry
        };
        if let tungstenite::Message::Text(text) = msg {
            if let Ok(resp) = serde_json::from_str::<Value>(&text) {
                if resp.get("id").and_then(|v| v.as_i64()) == Some(1) {
                    return Ok(resp["result"]["cookies"].as_array().map(|a| {
                        a.iter().map(|c| json!({
                            "name": c["name"], "value": c["value"],
                            "domain": c["domain"], "path": c["path"],
                        })).collect()
                    }).unwrap_or_default());
                }
            }
        }
    }
}

fn http_get(url: &str) -> Result<String, String> {
    let u: url::Url = url.parse().map_err(|e| format!("{}", e))?;
    let addr = format!("{}:{}", u.host_str().unwrap_or("127.0.0.1"), u.port().unwrap_or(80));
    let mut stream = TcpStream::connect(&addr).map_err(|e| format!("{}", e))?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let req = format!("GET {} HTTP/1.0\r\nHost: {}\r\n\r\n", u.path(), u.host_str().unwrap_or(""));
    stream.write_all(req.as_bytes()).map_err(|e| format!("{}", e))?;
    let mut reader = BufReader::new(stream);
    let mut body = String::new();
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {
                if line.trim().is_empty() { break; } // End of headers
            }
            Err(_) => break,
        }
    }
    reader.read_to_string(&mut body).ok();
    Ok(body)
}

// ═══════════════════════════════════════════════════════════════════
//  实例检查：同版本不重开，新版本强制重启
// ═══════════════════════════════════════════════════════════════════

fn check_existing_instance(port: &str) -> Option<bool> {
    // 返回值: None=无实例运行, Some(true)=已关闭旧进程, Some(false)=同版本运行中
    let url = format!("http://127.0.0.1:{}/api/stats", port);
    match http_get(&url) {
        Ok(body) => {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(remote) = v["build_version"].as_str() {
                    let my_ver = get_build_version();
                    if remote == my_ver {
                        info!("同版本实例已在运行 ({}), 跳过启动", remote);
                        return Some(false);
                    }
                    info!("发现旧版本实例 ({} → {}), 关闭旧进程", remote, my_ver);
                    kill_process_on_port(port);
                    return Some(true);
                }
            }
            // 无法解析版本，也关掉
            info!("发现不兼容实例, 关闭旧进程");
            kill_process_on_port(port);
            Some(true)
        }
        Err(_) => None,
    }
}

fn open_browser_url(url: &str) {
    let _ = std::process::Command::new("cmd").args(["/c", "start", "", url]).spawn();
}

fn kill_process_on_port(port: &str) {
    // 通过 netstat 查找端口对应的 PID
    let output = std::process::Command::new("cmd")
        .args([
            "/c",
            &format!("netstat -ano | findstr :{}", port),
        ])
        .output()
        .ok();
    if let Some(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            if line.contains("LISTENING") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(pid_str) = parts.last() {
                    if let Ok(pid) = pid_str.parse::<u32>() {
                        info!("正在关闭旧进程 PID={}", pid);
                        let _ = std::process::Command::new("taskkill")
                            .args(["/f", "/pid", &pid.to_string()])
                            .output();
                        // 等待端口释放
                        for _ in 0..10 {
                            if http_get(&format!("http://127.0.0.1:{}", port)).is_err() {
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(300));
                        }
                        return;
                    }
                }
            }
        }
    }
    // 保底：杀掉所有同名进程（一般不触发，因为新进程还未绑定端口）
    let _ = std::process::Command::new("taskkill")
        .args(["/f", "/im", "meituan-rs.exe"])
        .output();
    std::thread::sleep(Duration::from_secs(1));
}

// ═══════════════════════════════════════════════════════════════════
//  System Tray (Windows)
// ═══════════════════════════════════════════════════════════════════

#[cfg(windows)]
mod tray {
    use tray_icon::menu::*;
    use tray_icon::{Icon, TrayIconEvent};
    use log::info;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_OPEN_MS: AtomicU64 = AtomicU64::new(0);

    // Win32 message pump FFI
    mod win32 {
        #[repr(C)] pub struct POINT { pub x: i32, pub y: i32 }
        #[repr(C)] pub struct MSG { pub hwnd: isize, pub message: u32, pub wparam: usize, pub lparam: isize, pub time: u32, pub pt: POINT }
        extern "system" {
            pub fn GetMessageW(msg: *mut MSG, hwnd: isize, wMsgFilterMin: u32, wMsgFilterMax: u32) -> i32;
            pub fn TranslateMessage(msg: *const MSG) -> i32;
            pub fn DispatchMessageW(msg: *const MSG) -> isize;
        }
    }

    fn create_icon() -> Icon {
        let size = 32u32;
        let half = size as f32 / 2.0;
        let radius = half - 1.2;
        let mut rgba = Vec::with_capacity((size * size * 4) as usize);
        let (my0, my1, _ml, _mr, center) = (5.0, 27.0, 7.0, 25.0, 16.0);
        for y in 0..size {
            for x in 0..size {
                let (dx, dy) = (x as f32 - half, y as f32 - half);
                if (dx * dx + dy * dy).sqrt() > radius {
                    rgba.extend_from_slice(&[0,0,0,0]); continue;
                }
                let in_m = if (y as f32) < my0 || (y as f32) > my1 { false }
                    else if (x as f32) <= 11.0 || (x as f32) >= 21.0 { true }
                    else { let t = (y as f32 - my0)/(my1-my0); let vl = 11.0+(center-11.0)*t; let vr = 21.0-(21.0-center)*t; (x as f32) < vl || (x as f32) > vr };
                rgba.extend_from_slice(if in_m { &[255,255,255,255] } else { &[255,209,0,255] });
            }
        }
        Icon::from_rgba(rgba, size, size).expect("tray icon")
    }

    fn open_browser(port: &str, suffix: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // 使用 CAS 原子锁进行线程安全的防抖校验与更新，防止微秒级并发绕过
        let res = LAST_OPEN_MS.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |last| {
            if now >= last && now - last < 800 {
                None // 💡 800ms 内拒绝更新，触发拦截
            } else {
                Some(now) // 💡 允许更新为当前时间
            }
        });

        if res.is_err() {
            return; // 💡 拦截成功，直接退出
        }

        let url = format!("http://localhost:{}{}", port, suffix);
        // cmd /c start opens default browser, reuses existing tab
        let _ = std::process::Command::new("cmd").args(["/c","start","",&url]).spawn();
    }

    fn build_menu() -> Menu {
        let menu = Menu::new();
        let open = MenuItemBuilder::new().id(MenuId::new("open")).text("打开管理台").enabled(true).build();
        let settings_center = MenuItemBuilder::new().id(MenuId::new("settings_center")).text("打开设置中心").enabled(true).build();
        let refresh_fast = MenuItemBuilder::new().id(MenuId::new("refresh_fast")).text("快速同步（15分钟）").enabled(true).build();
        let refresh_deep = MenuItemBuilder::new().id(MenuId::new("refresh_deep")).text("深度对账（50小时）").enabled(true).build();
        let quick_settings = MenuItemBuilder::new().id(MenuId::new("quick_settings")).text("快速设置（原生）").enabled(true).build();
        let fee_settings = MenuItemBuilder::new().id(MenuId::new("fee_settings")).text("计费规则").enabled(true).build();
        let cookie_settings = MenuItemBuilder::new().id(MenuId::new("cookie_settings")).text("凭证与安全").enabled(true).build();
        let open_dir = MenuItemBuilder::new().id(MenuId::new("open_dir")).text("打开配置目录").enabled(true).build();
        let open_log = MenuItemBuilder::new().id(MenuId::new("open_log")).text("打开运行日志").enabled(true).build();
        let status = MenuItemBuilder::new().id(MenuId::new("status")).text("服务状态").enabled(true).build();
        let about = MenuItemBuilder::new().id(MenuId::new("about")).text("关于").enabled(true).build();
        let quit = MenuItemBuilder::new().id(MenuId::new("quit")).text("退出服务").enabled(true).build();

        let sync_menu = SubmenuBuilder::new()
            .text("数据同步")
            .enabled(true)
            .items(&[&refresh_fast, &refresh_deep])
            .build()
            .expect("sync submenu");
        let settings_menu = SubmenuBuilder::new()
            .text("设置")
            .enabled(true)
            .items(&[&settings_center, &quick_settings, &fee_settings, &cookie_settings])
            .build()
            .expect("settings submenu");
        let tools_menu = SubmenuBuilder::new()
            .text("工具")
            .enabled(true)
            .items(&[&status, &open_dir, &open_log])
            .build()
            .expect("tools submenu");

        let sep1 = PredefinedMenuItem::separator();
        let sep2 = PredefinedMenuItem::separator();
        let sep3 = PredefinedMenuItem::separator();
        let _ = menu.append_items(&[&open, &sep1, &sync_menu, &settings_menu, &tools_menu, &sep2, &about, &sep3, &quit]);
        menu
    }

    fn open_path(path: &str) {
        let _ = std::process::Command::new("cmd").args(["/c", "start", "", path]).spawn();
    }

    fn run_refresh_in_background(exe_dir: &str, deep: bool) {
        let exe_dir = exe_dir.to_string();
        std::thread::spawn(move || {
            let s = crate::rust_refresh(&exe_dir, deep);
            let msg = if s.errors.is_empty() {
                format!("同步完成\n+{} 新订单\n{} 条更新\n完成时间：{}", s.new, s.updated, s.time)
            } else {
                format!("同步未完成\n{}", s.errors.join("\n"))
            };
            show_msgbox("美团订单管理", &msg);
        });
    }

    fn handle_menu(event: &MenuEvent, exe_dir: &str, port: &str) {
        match event.id().as_ref() {
            "open" => open_browser(port, ""),
            "settings_center" => open_browser(port, "/#settings"),
            "fee_settings" => open_browser(port, "/#fee"),
            "cookie_settings" => open_browser(port, "/#settings-system"),
            "refresh_fast" => run_refresh_in_background(exe_dir, false),
            "refresh_deep" => run_refresh_in_background(exe_dir, true),
            "quick_settings" => {
                std::thread::spawn(|| unsafe { show_native_settings_dialog(); });
            }
            "status" => {
                show_msgbox("服务状态", &format!("服务运行中\n本机地址：http://localhost:{}\n配置目录：{}", port, exe_dir));
            }
            "open_dir" => open_path(exe_dir),
            "open_log" => open_path(&format!("{}\\meituan-rs.log", exe_dir)),
            "about" => {
                show_msgbox("关于", &format!("美团订单管理系统 v{}\n\nhttp://localhost:{}", env!("CARGO_PKG_VERSION"), port));
            }
            "quit" => {
                info!("用户退出");
                std::process::exit(0);
            }
            _ => {}
        }
    }

    fn show_msgbox(title: &str, msg: &str) {
        use std::os::windows::ffi::OsStrExt;
        use std::ffi::OsStr;
        let title_wide: Vec<u16> = OsStr::new(title).encode_wide().chain(std::iter::once(0)).collect();
        let msg_wide: Vec<u16> = OsStr::new(msg).encode_wide().chain(std::iter::once(0)).collect();
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                std::ptr::null_mut(),
                msg_wide.as_ptr(),
                title_wide.as_ptr(),
                0, // MB_OK
            );
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    //  原生 Win32 设置对话框 — 绝不打开浏览器，直接修改 settings.json
    // ═══════════════════════════════════════════════════════════════════

    /// 对话框控件 ID
    const IDC_EDIT_DAY_START: i32 = 1001;
    const IDC_EDIT_DAY_END: i32 = 1002;
    const IDC_EDIT_NIGHT_START: i32 = 1003;
    const IDC_EDIT_NIGHT_END: i32 = 1004;
    const IDC_EDIT_REFRESH: i32 = 1005;
    const IDC_CHECK_AUTO_OPEN: i32 = 1006;
    const IDC_CHECK_AUTO_LOGIN: i32 = 1007;
    const IDC_CHECK_MONTH_CAL: i32 = 1010;
    const IDC_CHECK_MONTH_PREV: i32 = 1011;
    const IDC_BTN_OK: i32 = 1012;
    const IDC_BTN_CANCEL: i32 = 1013;

    // Win32 常量
    const GWLP_USERDATA: i32 = -21;
    const BST_CHECKED: usize = 0x0001;
    const BM_GETCHECK: u32 = 0x00F0;
    const BM_SETCHECK: u32 = 0x00F1;

    /// 对话框上下文（放置于堆上，通过 GWLP_USERDATA 传递给窗口过程）
    struct DlgCtx {
        settings: *mut crate::Settings,
        result: *mut bool,
        done: *mut bool,
        hwnd_day_start: windows_sys::Win32::Foundation::HWND,
        hwnd_day_end: windows_sys::Win32::Foundation::HWND,
        hwnd_night_start: windows_sys::Win32::Foundation::HWND,
        hwnd_night_end: windows_sys::Win32::Foundation::HWND,
        hwnd_refresh: windows_sys::Win32::Foundation::HWND,
        hwnd_auto_open: windows_sys::Win32::Foundation::HWND,
        hwnd_auto_login: windows_sys::Win32::Foundation::HWND,
        hwnd_month_cal: windows_sys::Win32::Foundation::HWND,
        hwnd_month_prev: windows_sys::Win32::Foundation::HWND,
    }

    unsafe extern "system" fn dlg_wnd_proc(hwnd: windows_sys::Win32::Foundation::HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
        match msg {
            // WM_CREATE = 0x0001
            0x0001 => {
                let create = &*(lparam as *const windows_sys::Win32::UI::WindowsAndMessaging::CREATESTRUCTW);
                let ctx = Box::from_raw((*create).lpCreateParams as *mut DlgCtx);

                // 保存 ctx 到窗口属性
                windows_sys::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(ctx) as isize);
                0
            }
            // WM_COMMAND = 0x0111
            0x0111 => {
                let id = (wparam & 0xFFFF) as i32;
                let ctx_ptr = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DlgCtx;
                if !ctx_ptr.is_null() {
                    let ctx = &mut *ctx_ptr;
                    if id == IDC_BTN_OK {
                        save_settings_from_dialog(ctx);
                        *ctx.result = true;
                        windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
                        return 0;
                    }
                    if id == IDC_BTN_CANCEL {
                        *ctx.result = false;
                        windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
                        return 0;
                    }
                }
                // 让 DefWindowProc 也处理
                windows_sys::Win32::UI::WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            // WM_CLOSE = 0x0010
            0x0010 => {
                let ctx_ptr = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DlgCtx;
                if !ctx_ptr.is_null() {
                    let ctx = &mut *ctx_ptr;
                    *ctx.result = false;
                }
                windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
                0
            }
            // WM_DESTROY = 0x0002
            0x0002 => {
                // 结束当前设置对话框循环，但不退出托盘主消息循环
                let ctx_ptr = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DlgCtx;
                if !ctx_ptr.is_null() {
                    let ctx = &mut *ctx_ptr;
                    *ctx.done = true;
                    let _ = Box::from_raw(ctx_ptr);
                    windows_sys::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                0
            }
            _ => windows_sys::Win32::UI::WindowsAndMessaging::DefWindowProcW(hwnd, msg as u32, wparam, lparam),
        }
    }

    /// 从对话框读取值并保存到 settings
    unsafe fn save_settings_from_dialog(ctx: &mut DlgCtx) {
        let s = &mut *ctx.settings;

        // 读取 Edit 控件文本（辅助函数）
        unsafe fn read_edit(hwnd: windows_sys::Win32::Foundation::HWND) -> String {
            let len = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextLengthW(hwnd);
            if len <= 0 { return String::new(); }
            let mut buf = vec![0u16; (len + 1) as usize];
            windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
            let slice = &buf[..len as usize];
            String::from_utf16_lossy(slice)
        }

        // 读取班次时间
        let ds = read_edit(ctx.hwnd_day_start);
        let de = read_edit(ctx.hwnd_day_end);
        let ns = read_edit(ctx.hwnd_night_start);
        let ne = read_edit(ctx.hwnd_night_end);

        if let Some((h, m)) = parse_hm(&ds) { s.shift.day_start = vec![h, m]; }
        if let Some((h, m)) = parse_hm(&de) { s.shift.day_end = vec![h, m]; }
        if let Some((h, m)) = parse_hm(&ns) { s.shift.night_start = vec![h, m]; }
        if let Some((h, m)) = parse_hm(&ne) { s.shift.night_end = vec![h, m]; }

        // 读取刷新间隔
        let refresh_str = read_edit(ctx.hwnd_refresh);
        if let Ok(secs) = refresh_str.trim().parse::<i64>() {
            if secs >= 5 && secs <= 3600 {
                s.refresh_interval_secs = secs;
            }
        }

        // 读取系统开关
        s.auto_open_browser = windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(
            ctx.hwnd_auto_open, BM_GETCHECK, 0, 0
        ) == BST_CHECKED as isize;
        s.auto_login = windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(
            ctx.hwnd_auto_login, BM_GETCHECK, 0, 0
        ) == BST_CHECKED as isize;
        s.month_start_cal = windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(
            ctx.hwnd_month_cal, BM_GETCHECK, 0, 0
        ) == BST_CHECKED as isize;
        s.month_end_prev = windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(
            ctx.hwnd_month_prev, BM_GETCHECK, 0, 0
        ) == BST_CHECKED as isize;

        // 写入 settings.json
        let _ = crate::save_settings(s);
        info!("设置已保存: refresh={}s, auto_open={}, shift={:?}",
            s.refresh_interval_secs, s.auto_open_browser, s.shift);
    }

    /// 解析 "08:00" 格式的字符串为 (小时, 分钟)
    fn parse_hm(s: &str) -> Option<(i32, i32)> {
        let parts: Vec<&str> = s.trim().split(':').collect();
        if parts.len() == 2 {
            if let (Ok(h), Ok(m)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                if h >= 0 && h <= 23 && m >= 0 && m <= 59 {
                    return Some((h, m));
                }
            }
        }
        None
    }

    /// 显示原生 Win32 设置对话框（绝不打开浏览器）
    unsafe fn show_native_settings_dialog() {
        use std::os::windows::ffi::OsStrExt;
        use std::ffi::OsStr;
        use std::mem;

        fn to_wstr(s: &str) -> Vec<u16> {
            OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
        }

        // 加载当前设置
        let mut settings = crate::load_settings();

        // 注册窗口类
        let h_instance = windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null_mut());
        let cls_name = to_wstr("MeituanSettingsDlg");
        let wc = windows_sys::Win32::UI::WindowsAndMessaging::WNDCLASSEXW {
            cbSize: mem::size_of::<windows_sys::Win32::UI::WindowsAndMessaging::WNDCLASSEXW>() as u32,
            style: windows_sys::Win32::UI::WindowsAndMessaging::CS_HREDRAW | windows_sys::Win32::UI::WindowsAndMessaging::CS_VREDRAW,
            lpfnWndProc: Some(dlg_wnd_proc),
            hInstance: h_instance,
            hCursor: windows_sys::Win32::UI::WindowsAndMessaging::LoadCursorW(std::ptr::null_mut(), windows_sys::Win32::UI::WindowsAndMessaging::IDC_ARROW),
            hbrBackground: std::ptr::null_mut(),
            lpszClassName: cls_name.as_ptr(),
            ..mem::zeroed()
        };
        windows_sys::Win32::UI::WindowsAndMessaging::RegisterClassExW(&wc);

        // 计算窗口居中位置
        let screen_w = windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(0);  // SM_CXSCREEN
        let screen_h = windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(1);  // SM_CYSCREEN
        let dlg_w = 560i32;
        let dlg_h = 520i32;
        let x = (screen_w - dlg_w) / 2;
        let y = (screen_h - dlg_h) / 2;

        // 初始化 DlgCtx
        let result_ptr = Box::into_raw(Box::new(false));
        let done_ptr = Box::into_raw(Box::new(false));
        let ctx = Box::new(DlgCtx {
            settings: &mut settings,
            result: result_ptr,
            done: done_ptr,
            hwnd_day_start: std::ptr::null_mut(),
            hwnd_day_end: std::ptr::null_mut(),
            hwnd_night_start: std::ptr::null_mut(),
            hwnd_night_end: std::ptr::null_mut(),
            hwnd_refresh: std::ptr::null_mut(),
            hwnd_auto_open: std::ptr::null_mut(),
            hwnd_auto_login: std::ptr::null_mut(),
            hwnd_month_cal: std::ptr::null_mut(),
            hwnd_month_prev: std::ptr::null_mut(),
        });
        let ctx_ptr = Box::into_raw(ctx);

        // 创建对话框主窗口
        let title = to_wstr("⚙️ 系统设置 — 美团订单管理");
        let hwnd = windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_DLGMODALFRAME,
            cls_name.as_ptr(),
            title.as_ptr(),
            windows_sys::Win32::UI::WindowsAndMessaging::WS_POPUP | windows_sys::Win32::UI::WindowsAndMessaging::WS_CAPTION | windows_sys::Win32::UI::WindowsAndMessaging::WS_SYSMENU,
            x, y, dlg_w, dlg_h,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            h_instance,
            ctx_ptr as *const std::ffi::c_void,
        );

        if hwnd.is_null() {
            // 窗口创建失败，释放 ctx 和结果指针
            let _ = Box::from_raw(ctx_ptr);
            let _ = Box::from_raw(result_ptr);
            let _ = Box::from_raw(done_ptr);
            return;
        }

        // 创建子控件（使用 DS_SHELLFONT 风格并设置字体）
        let child_style = windows_sys::Win32::UI::WindowsAndMessaging::WS_CHILD | windows_sys::Win32::UI::WindowsAndMessaging::WS_VISIBLE;
        let label_style = child_style;
        let btn_checkbox_style = child_style | windows_sys::Win32::UI::WindowsAndMessaging::BS_AUTOCHECKBOX as u32;
        let edit_style = child_style | windows_sys::Win32::UI::WindowsAndMessaging::WS_BORDER | windows_sys::Win32::UI::WindowsAndMessaging::ES_AUTOHSCROLL as u32;
        let btn_style = child_style | windows_sys::Win32::UI::WindowsAndMessaging::BS_DEFPUSHBUTTON as u32;

        // 获取 ctx 来存储控件句柄
        let ctx_ref = &mut *ctx_ptr;

        let mut cy = 16i32;
        let lx = 24i32;
        let ex = 260i32;
        let ew = 220i32;
        let lh = 22i32;

        // ═══ 班次设置 ═══
        let _fc = windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            0, to_wstr("STATIC").as_ptr(), to_wstr("── 班次时间设置 ──").as_ptr(),
            label_style, lx, cy, 340, lh, hwnd, std::ptr::null_mut(), h_instance, std::ptr::null_mut());
        cy += lh + 4;

        // 白班开始
        windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            0, to_wstr("STATIC").as_ptr(), to_wstr("白班开始时间 (HH:MM):").as_ptr(),
            label_style, lx, cy, 170, lh, hwnd, std::ptr::null_mut(), h_instance, std::ptr::null_mut());
        ctx_ref.hwnd_day_start = windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_CLIENTEDGE, to_wstr("EDIT").as_ptr(),
            to_wstr(&format!("{:02}:{:02}", settings.shift.day_start[0], settings.shift.day_start[1])).as_ptr(),
            edit_style | windows_sys::Win32::UI::WindowsAndMessaging::ES_CENTER as u32, ex, cy, ew, lh + 4, hwnd, IDC_EDIT_DAY_START as usize as *mut std::ffi::c_void, h_instance, std::ptr::null_mut());
        cy += lh + 8;

        // 白班结束
        windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            0, to_wstr("STATIC").as_ptr(), to_wstr("白班结束时间 (HH:MM):").as_ptr(),
            label_style, lx, cy, 170, lh, hwnd, std::ptr::null_mut(), h_instance, std::ptr::null_mut());
        ctx_ref.hwnd_day_end = windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_CLIENTEDGE, to_wstr("EDIT").as_ptr(),
            to_wstr(&format!("{:02}:{:02}", settings.shift.day_end[0], settings.shift.day_end[1])).as_ptr(),
            edit_style | windows_sys::Win32::UI::WindowsAndMessaging::ES_CENTER as u32, ex, cy, ew, lh + 4, hwnd, IDC_EDIT_DAY_END as usize as *mut std::ffi::c_void, h_instance, std::ptr::null_mut());
        cy += lh + 8;

        // 夜班开始
        windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            0, to_wstr("STATIC").as_ptr(), to_wstr("夜班开始时间 (HH:MM):").as_ptr(),
            label_style, lx, cy, 170, lh, hwnd, std::ptr::null_mut(), h_instance, std::ptr::null_mut());
        ctx_ref.hwnd_night_start = windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_CLIENTEDGE, to_wstr("EDIT").as_ptr(),
            to_wstr(&format!("{:02}:{:02}", settings.shift.night_start[0], settings.shift.night_start[1])).as_ptr(),
            edit_style | windows_sys::Win32::UI::WindowsAndMessaging::ES_CENTER as u32, ex, cy, ew, lh + 4, hwnd, IDC_EDIT_NIGHT_START as usize as *mut std::ffi::c_void, h_instance, std::ptr::null_mut());
        cy += lh + 8;

        // 夜班结束
        windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            0, to_wstr("STATIC").as_ptr(), to_wstr("夜班结束时间 (HH:MM):").as_ptr(),
            label_style, lx, cy, 170, lh, hwnd, std::ptr::null_mut(), h_instance, std::ptr::null_mut());
        ctx_ref.hwnd_night_end = windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_CLIENTEDGE, to_wstr("EDIT").as_ptr(),
            to_wstr(&format!("{:02}:{:02}", settings.shift.night_end[0], settings.shift.night_end[1])).as_ptr(),
            edit_style | windows_sys::Win32::UI::WindowsAndMessaging::ES_CENTER as u32, ex, cy, ew, lh + 4, hwnd, IDC_EDIT_NIGHT_END as usize as *mut std::ffi::c_void, h_instance, std::ptr::null_mut());
        cy += lh + 12;

        // ═══ 系统设置 ═══
        windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            0, to_wstr("STATIC").as_ptr(), to_wstr("── 后端系统配置 ──").as_ptr(),
            label_style, lx, cy, 340, lh, hwnd, std::ptr::null_mut(), h_instance, std::ptr::null_mut());
        cy += lh + 4;

        // 刷新间隔标签 + 输入
        windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            0, to_wstr("STATIC").as_ptr(), to_wstr("自动同步间隔 (秒, 5~3600):").as_ptr(),
            label_style, lx, cy, 170, lh, hwnd, std::ptr::null_mut(), h_instance, std::ptr::null_mut());
        ctx_ref.hwnd_refresh = windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_CLIENTEDGE, to_wstr("EDIT").as_ptr(),
            to_wstr(&settings.refresh_interval_secs.to_string()).as_ptr(),
            edit_style, ex, cy, ew, lh + 4, hwnd, IDC_EDIT_REFRESH as usize as *mut std::ffi::c_void, h_instance, std::ptr::null_mut());
        cy += lh + 8;

        // 系统开关
        ctx_ref.hwnd_auto_open = windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            0, to_wstr("BUTTON").as_ptr(),
            to_wstr("启动时自动打开管理台网页").as_ptr(),
            btn_checkbox_style, lx, cy, 420, lh + 4, hwnd, IDC_CHECK_AUTO_OPEN as usize as *mut std::ffi::c_void, h_instance, std::ptr::null_mut());
        if settings.auto_open_browser {
            windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(ctx_ref.hwnd_auto_open, BM_SETCHECK, BST_CHECKED, 0);
        }
        cy += lh + 6;

        ctx_ref.hwnd_auto_login = windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            0, to_wstr("BUTTON").as_ptr(),
            to_wstr("自动提取 Chrome/Edge 商家凭证").as_ptr(),
            btn_checkbox_style, lx, cy, 420, lh + 4, hwnd, IDC_CHECK_AUTO_LOGIN as usize as *mut std::ffi::c_void, h_instance, std::ptr::null_mut());
        if settings.auto_login {
            windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(ctx_ref.hwnd_auto_login, BM_SETCHECK, BST_CHECKED, 0);
        }
        cy += lh + 14;

        windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            0, to_wstr("STATIC").as_ptr(), to_wstr("── 本月统计边界 ──").as_ptr(),
            label_style, lx, cy, 480, lh, hwnd, std::ptr::null_mut(), h_instance, std::ptr::null_mut());
        cy += lh + 4;

        ctx_ref.hwnd_month_cal = windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            0, to_wstr("BUTTON").as_ptr(),
            to_wstr("本月从 00:00 日历日开始统计").as_ptr(),
            btn_checkbox_style, lx, cy, 420, lh + 4, hwnd, IDC_CHECK_MONTH_CAL as usize as *mut std::ffi::c_void, h_instance, std::ptr::null_mut());
        if settings.month_start_cal {
            windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(ctx_ref.hwnd_month_cal, BM_SETCHECK, BST_CHECKED, 0);
        }
        cy += lh + 6;

        ctx_ref.hwnd_month_prev = windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            0, to_wstr("BUTTON").as_ptr(),
            to_wstr("本月统计不包含当前班次").as_ptr(),
            btn_checkbox_style, lx, cy, 420, lh + 4, hwnd, IDC_CHECK_MONTH_PREV as usize as *mut std::ffi::c_void, h_instance, std::ptr::null_mut());
        if settings.month_end_prev {
            windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(ctx_ref.hwnd_month_prev, BM_SETCHECK, BST_CHECKED, 0);
        }
        cy += lh + 18;

        windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            0, to_wstr("STATIC").as_ptr(),
            to_wstr("完整列显示、计费规则和 Cookie 粘贴请从托盘打开“设置中心”。").as_ptr(),
            label_style, lx, cy, 500, lh, hwnd, std::ptr::null_mut(), h_instance, std::ptr::null_mut());
        cy += lh + 16;

        // ═══ 按钮 ═══
        let bw = 112i32;
        let bh = 30i32;
        let bx_ok = dlg_w - bw * 2 - 44;
        let bx_cancel = bx_ok + bw + 16;

        windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            0, to_wstr("BUTTON").as_ptr(), to_wstr("保存").as_ptr(),
            btn_style, bx_ok, cy, bw, bh, hwnd, IDC_BTN_OK as usize as *mut std::ffi::c_void, h_instance, std::ptr::null_mut());
        windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            0, to_wstr("BUTTON").as_ptr(), to_wstr("取消").as_ptr(),
            child_style | windows_sys::Win32::UI::WindowsAndMessaging::BS_PUSHBUTTON as u32, bx_cancel, cy, bw, bh, hwnd, IDC_BTN_CANCEL as usize as *mut std::ffi::c_void, h_instance, std::ptr::null_mut());

        // 显示窗口
        windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(hwnd, 1);  // SW_SHOWNORMAL

        // 消息循环（独立的模态循环，不影响托盘主循环）
        let mut msg: windows_sys::Win32::UI::WindowsAndMessaging::MSG = mem::zeroed();
        while !*done_ptr && windows_sys::Win32::UI::WindowsAndMessaging::GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) != 0 {
            windows_sys::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
            windows_sys::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
        }

        let saved = *Box::from_raw(result_ptr);
        let _ = Box::from_raw(done_ptr);
        if saved {
            info!("✅ 设置已保存: refresh={}s, auto_open={}", settings.refresh_interval_secs, settings.auto_open_browser);
        }
    }

    pub fn run(_exe_dir: String, _html_dir: String, port: String) {
        let icon = create_icon();
        let menu = build_menu();
        let _tray = match tray_icon::TrayIconBuilder::new()
            .with_tooltip("美团订单管理 - 双击打开网页")
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .build()
        {
            Ok(t) => { info!("系统托盘已创建"); t }
            Err(e) => { log::error!("托盘创建失败: {}", e); return; }
        };

        unsafe {
            let mut msg: win32::MSG = std::mem::zeroed();
            // GetMessageW 阻塞等待消息，系统休眠挂起，零 CPU 占用，极速唤醒响应
            while win32::GetMessageW(&mut msg, 0, 0, 0) != 0 {
                win32::TranslateMessage(&msg);
                win32::DispatchMessageW(&msg);

                // 消息循环分发后，立刻消费 Rust 跨平台通道中被填充的事件
                while let Ok(event) = MenuEvent::receiver().try_recv() {
                    handle_menu(&event, &_exe_dir, &port);
                }
                while let Ok(event) = TrayIconEvent::receiver().try_recv() {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: tray_icon::MouseButton::Left,
                            button_state: tray_icon::MouseButtonState::Up,
                            ..
                        }
                    ) {
                        open_browser(&port, "");
                    }
                }
            }
        }
    }
}

#[cfg(not(windows))]
mod tray {
    pub fn run(_exe_dir: String, _html_dir: String, _port: String) {
        log::info!("系统托盘: 当前平台不支持");
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Main
// ═══════════════════════════════════════════════════════════════════

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 获取工作目录和端口
    let exe_dir = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let port = std::env::var("PORT").unwrap_or_else(|_| "8899".into());

    // 初始化日志系统
    init_logging(&exe_dir);

    let version = env!("CARGO_PKG_VERSION");
    let build_time = get_build_version();

    // 启动横幅
    info!("");
    info!("╔══════════════════════════════════════════════════════════════╗");
    info!("║  🛒  美团订单管理系统 v{} ({})", version, build_time);
    info!("╚══════════════════════════════════════════════════════════════╝");
    info!("");

    let dir = exe_dir.clone();

    // 🔐 密码验证（首次设置或登录）
    let key = {
        let meta_path = password_meta_path();
        if !std::path::Path::new(&meta_path).exists() {
            // 首次使用，设置密码
            let (password, meta) = setup_password();
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            let salt = crate::crypto::random_bytes::<16>();
            let key = derive_key(&password, &salt);
            // 初始化元数据（不包含加密 Cookie）
            let meta_json = serde_json::to_string_pretty(&meta).unwrap();
            std::fs::write(&meta_path, meta_json).unwrap();
            key
        } else {
            // 验证密码
            let meta_json = std::fs::read_to_string(&meta_path).unwrap();
            let meta: PasswordMeta = serde_json::from_str(&meta_json).unwrap();
            verify_password(&meta)
        }
    };

    // 检查已有实例
    match check_existing_instance(&port) {
        Some(false) => {
            info!("已在运行 → http://localhost:{}", port);
            info!("");
            std::process::exit(0);
        }
        Some(true) => {
            info!("旧版本已关闭，启动新版本...");
        }
        None => {}
    }

    println!("  📋 [1/4] 检查登录状态...");
    let auto_login = load_settings().auto_login && std::env::var("AUTO_LOGIN").map(|v| v != "0" && v != "false").unwrap_or(true);
    let has_cookies = if auto_login {
        print!("         └─ 正在从浏览器提取 Cookie...");
        let ok = std::thread::spawn(ensure_cookies).join().unwrap_or(false);
        if ok { println!(" ✅"); } else { println!(" ⚠️ 需要登录"); }
        ok
    } else {
        let cpath = cookie_file_path();
        let ok = std::path::Path::new(&cpath).exists();
        if ok { println!("         └─ Cookie 已存在 ✅"); }
        ok
    };
    if !has_cookies {
        println!("         └─ ⚠️  Cookie 缺失。首次使用请确保已登录美团商家后台");
    }

     println!("");
     println!("  📋 [2/4] 初始化数据库...");
     let enc_key_hex = key.iter().map(|b| format!("{:02x}", b)).collect::<String>();
     let manager = SqliteConnectionManager::file(format!("{}/meituan_orders.db", exe_dir))
         .with_init(move |conn| {
             conn.execute(&format!("PRAGMA key = \"x'{}'\";", enc_key_hex), [])
                 .map(|_| ())
         });
    let pool = Pool::builder()
        .max_size(8)
        .connection_timeout(Duration::from_secs(5))
        .build(manager)
        .expect("数据库连接池初始化失败");
    init_db(&pool).expect("数据库表初始化完成");
    let conn = pool.get().expect("获取数据库连接失败");
    let total = db_total(&conn);
    let refunded = db_refunded(&conn);
    let (min_date, max_date) = db_daterange(&conn);
    println!("         └─ 共 {} 条订单 | 已退款 {} 条", total, refunded);
    println!("         └─ 数据范围: {} ~ {}", min_date.as_deref().unwrap_or("-"), max_date.as_deref().unwrap_or("-"));

    println!("");
    println!("  📋 [3/4] 检查配置文件...");
    let cookie_ok = std::path::Path::new(&enc_cookie_path()).exists();
    println!("         └─ Cookie 凭证: {}", if cookie_ok { "✅" } else { "⚠️ 缺失" });
    if std::path::Path::new(&format!("{}/settings.json", exe_dir)).exists() {
        let s = load_settings();
        let fee_count = serde_json::from_str::<Vec<serde_json::Value>>(&s.fee_json).unwrap_or_default().len();
        println!("         └─ 计费规则: ✅ ({} 条套餐)", fee_count);
    } else {
        println!("         └─ 计费规则: ⚠️ 使用默认");
    }

    println!("");
    println!("  📋 [4/4] 启动服务...");

    let state = web::Data::new(AppState {
        db: pool.clone(),
        cookie_file: enc_cookie_path(),
        html_dir: dir.clone(),
        exe_dir: exe_dir.clone(),
        enc_key: key,
    });

		    // 自动更新任务：按 settings.json 中的刷新间隔拉取新数据 (使用 15分钟 短查找窗口快速同步最新核销与撤销状态)
		    {
		        let dir2 = exe_dir.clone();
		        actix_web::rt::spawn(async move {
		            loop {
		                let secs = load_settings().refresh_interval_secs.clamp(5, 3600) as u64;
		                actix_web::rt::time::sleep(Duration::from_secs(secs)).await;
	                let d = dir2.clone();
	                let result = tokio::task::spawn_blocking(move || {
	                    rust_refresh(&d, false)
	                }).await.unwrap_or_else(|e| {
	                    RefreshResponse { new: 0, updated: 0, time: "".into(), errors: vec![format!("spawn_blocking: {}", e)] }
	                });
                if !result.errors.is_empty() {
                    println!("  ⚠️  同步出错: {:?}", result.errors);
                } else if result.new > 0 || result.updated > 0 {
                    println!("  🔄 同步: +{} 新订单, {} 更新", result.new, result.updated);
                }
	            }
	        });
	    }

	    // 深度对账同步任务：固定 30 分钟间隔运行 (使用 50小时 深度查找窗口同步 48小时自动退款状态)
	    {
	        let dir2 = exe_dir.clone();
	        actix_web::rt::spawn(async move {
	            loop {
	                actix_web::rt::time::sleep(Duration::from_secs(1800)).await;
	                let d = dir2.clone();
	                let result = tokio::task::spawn_blocking(move || {
	                    rust_refresh(&d, true)
	                }).await.unwrap_or_else(|e| {
	                    RefreshResponse { new: 0, updated: 0, time: "".into(), errors: vec![format!("spawn_blocking deep: {}", e)] }
	                });
                if !result.errors.is_empty() {
                    println!("  ⚠️  深度同步出错: {:?}", result.errors);
                } else if result.new > 0 || result.updated > 0 {
                    println!("  🔄 深度同步: +{} 新订单, {} 更新", result.new, result.updated);
                }
	            }
	        });
	    }

            println!("         └─ HTTP 服务: ✅ 端口 {}", port);
            println!("");
            println!("  ┌──────────────────────────────────────────────────────────────┐");
            println!("  │  🌐 浏览器打开: http://localhost:{:<28} │", port);
            println!("  │  📱 局域网访问: http://<本机IP>:{:<28} │", port);
            println!("  │  ❌ 按 Ctrl+C 退出                                        │");
            println!("  └──────────────────────────────────────────────────────────────┘");
            println!("");

			    // 启动系统托盘（独立线程）
			    {
			        let tp = port.clone();
			        std::thread::spawn(move || {
			        tray::run(exe_dir.clone(), dir.clone(), tp);
			        });
				        info!("系统托盘线程已启动");
			    }

            if load_settings().auto_open_browser {
                let open_port = port.clone();
                actix_web::rt::spawn(async move {
                    actix_web::rt::time::sleep(Duration::from_millis(800)).await;
                    open_browser_url(&format!("http://localhost:{}", open_port));
                });
            }

			    match HttpServer::new(move || {
	        App::new()
	            .app_data(state.clone())
            .route("/", web::get().to(handle_index))
            .route("/settings", web::get().to(handle_settings_page))
            .route("/logo.png", web::get().to(handle_logo))
            .route("/api/health", web::get().to(handle_health))
	            .route("/api/stats", web::get().to(handle_stats))
	            .route("/api/stats/detail", web::get().to(handle_stats_detail))
	            .route("/api/query", web::get().to(handle_query))
	            .route("/api/refresh", web::get().to(handle_refresh))
            .route("/api/settings", web::get().to(handle_get_settings))
            .route("/api/settings", web::put().to(handle_put_settings))
            .route("/api/cookie/validate", web::get().to(handle_validate_cookies))
	    })
	    .workers(4)
	    .bind(format!("0.0.0.0:{}", port))
		    {
		        Ok(s) => {
		            s.run().await
		        }
	        Err(e) => {
                println!("");
                println!("  ❌ 端口 {} 绑定失败: {}", port, e);
                println!("  💡 更换端口: set PORT=8888 && meituan-rs.exe");
                println!("");
	            std::process::exit(1);
	        }
	    }
	}
