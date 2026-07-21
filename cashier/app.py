import sqlite3, json, time, os
from datetime import datetime, date
from flask import Flask, request, jsonify, render_template, g

app = Flask(__name__)
DB_PATH = os.path.join(os.path.dirname(__file__), "cashier.db")

# ═══════════════════════════════════════════════════════════════════════
#  Database
# ═══════════════════════════════════════════════════════════════════════

def get_db():
    db = getattr(g, "_db", None)
    if db is None:
        db = g._db = sqlite3.connect(DB_PATH)
        db.row_factory = sqlite3.Row
        db.execute("PRAGMA foreign_keys = ON")
        db.execute("PRAGMA journal_mode = WAL")
    return db

@app.teardown_appcontext
def close_db(exc):
    db = getattr(g, "_db", None)
    if db is not None:
        db.close()

def init_db():
    conn = sqlite3.connect(DB_PATH)
    conn.executescript("""
    -- 班次主表
    CREATE TABLE IF NOT EXISTS shifts (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        shift_date TEXT NOT NULL,           -- '2027-01-15'
        shift_type TEXT NOT NULL,           -- '白班' / '晚班'
        operator TEXT DEFAULT '',           -- 收银员
        meituan REAL DEFAULT 0,            -- 美团收入
        network_fee REAL DEFAULT 0,        -- 网费
        network_fee_due REAL DEFAULT 0,    -- 应缴网费
        sales REAL DEFAULT 0,             -- 售货总额
        expense_total REAL DEFAULT 0,      -- 支出合计
        refund_total REAL DEFAULT 0,       -- 退款/入账合计
        cash_begin REAL DEFAULT 0,        -- 开班现金
        cash_end REAL DEFAULT 0,          -- 交班现金
        notes TEXT DEFAULT '',
        status TEXT DEFAULT 'open',       -- open / closed
        created_at TEXT DEFAULT (datetime('now','localtime')),
        updated_at TEXT DEFAULT (datetime('now','localtime'))
    );

    -- 售货明细（商品清单）
    CREATE TABLE IF NOT EXISTS sales_items (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        shift_id INTEGER NOT NULL,
        product_name TEXT NOT NULL,
        begin_stock REAL DEFAULT 0,        -- 开班库存
        restock REAL DEFAULT 0,            -- 进货
        sold REAL DEFAULT 0,               -- 售出
        end_stock REAL DEFAULT 0,          -- 剩余
        unit_price REAL DEFAULT 0,         -- 单价
        capacity REAL DEFAULT 0,           -- 容量/规格
        sort_order INTEGER DEFAULT 0,
        FOREIGN KEY (shift_id) REFERENCES shifts(id) ON DELETE CASCADE
    );

    -- 支出明细
    CREATE TABLE IF NOT EXISTS expenses (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        shift_id INTEGER NOT NULL,
        item TEXT DEFAULT '',              -- 支出项目
        amount REAL DEFAULT 0,             -- 支出金额
        payment_type TEXT DEFAULT '',      -- 台付/线上/财务支付
        sort_order INTEGER DEFAULT 0,
        FOREIGN KEY (shift_id) REFERENCES shifts(id) ON DELETE CASCADE
    );

    -- 入账/退款明细
    CREATE TABLE IF NOT EXISTS income_items (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        shift_id INTEGER NOT NULL,
        item TEXT DEFAULT '',              -- 入账项目
        amount REAL DEFAULT 0,             -- 入账金额
        sort_order INTEGER DEFAULT 0,
        FOREIGN KEY (shift_id) REFERENCES shifts(id) ON DELETE CASCADE
    );

    -- 交易快照（每笔交易记录）
    CREATE TABLE IF NOT EXISTS transactions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        shift_id INTEGER NOT NULL,
        seq INTEGER DEFAULT 0,             -- 序号
        product_name TEXT DEFAULT '',
        coupon REAL DEFAULT 0,             -- 券面
        actual_paid REAL DEFAULT 0,        -- 实付金额
        merchant_subsidy REAL DEFAULT 0,   -- 商家优惠
        duration TEXT DEFAULT '',          -- 上网时长
        user_phone TEXT DEFAULT '',        -- 用户手机
        remark TEXT DEFAULT '',
        verify_code TEXT DEFAULT '',       -- 核销码
        fee_type TEXT DEFAULT '',          -- 费用类型
        sort_order INTEGER DEFAULT 0,
        FOREIGN KEY (shift_id) REFERENCES shifts(id) ON DELETE CASCADE
    );

    -- 版本记录（每次修改存快照）
    CREATE TABLE IF NOT EXISTS shift_versions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        shift_id INTEGER NOT NULL,
        version_num INTEGER DEFAULT 1,
        snapshot TEXT NOT NULL,            -- JSON 快照
        action TEXT DEFAULT 'create',      -- create/update/close/rollback
        operator TEXT DEFAULT '',
        created_at TEXT DEFAULT (datetime('now','localtime')),
        FOREIGN KEY (shift_id) REFERENCES shifts(id) ON DELETE CASCADE
    );

    -- 商品模板（默认商品列表，新建班次时自动填充）
    CREATE TABLE IF NOT EXISTS product_templates (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        product_name TEXT NOT NULL UNIQUE,
        unit_price REAL DEFAULT 0,
        capacity REAL DEFAULT 0,
        sort_order INTEGER DEFAULT 0
    );

    -- 员工列表（收银员）
    CREATE TABLE IF NOT EXISTS employees (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL UNIQUE,
        shift_type TEXT DEFAULT '',         -- 白班/晚班/全天
        phone TEXT DEFAULT '',
        sort_order INTEGER DEFAULT 0
    );

    -- 网费套餐（美团套餐表）
    CREATE TABLE IF NOT EXISTS fee_plans (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        category TEXT NOT NULL,            -- 新会员/5070显卡/网游区/普通区/老会员等
        plan_name TEXT NOT NULL,           -- 特惠/3小时/包天等
        price REAL DEFAULT 0,
        sort_order INTEGER DEFAULT 0,
        UNIQUE(category, plan_name)
    );

    -- 索引
    CREATE INDEX IF NOT EXISTS idx_shifts_date ON shifts(shift_date);
    CREATE INDEX IF NOT EXISTS idx_shifts_type ON shifts(shift_type);
    CREATE INDEX IF NOT EXISTS idx_sales_shift ON sales_items(shift_id);
    CREATE INDEX IF NOT EXISTS idx_expenses_shift ON expenses(shift_id);
    CREATE INDEX IF NOT EXISTS idx_income_shift ON income_items(shift_id);
    CREATE INDEX IF NOT EXISTS idx_txn_shift ON transactions(shift_id);
    CREATE INDEX IF NOT EXISTS idx_versions_shift ON shift_versions(shift_id);
    """)
    conn.commit()
    conn.close()

def seed_products():
    conn = sqlite3.connect(DB_PATH)
    products = [
        ("武夷山", 0, 0, 1),
        ("可口可乐", 3, 24, 2),
        ("百瓶/雪碧", 5, 12, 3),
        ("矿泉水小瓶", 5, 15, 4),
        ("营养快线", 5, 15, 5),
        ("红牛", 5, 15, 6),
        ("鸡蛋", 6, 24, 7),
        ("毛尖", 6, 12, 8),
        ("槟榔", 6, 24, 9),
        ("桶装泡面大", 6, 12, 10),
        ("袋装泡面盒", 5, 1, 11),
        ("矿泉水中瓶", 6, 12, 12),
        ("饮料", 6, 24, 13),
        ("矿泉水大瓶", 8, 15, 14),
        ("矿泉水特惠", 8, 12, 15),
        ("桶装泡面小", 6, 1, 16),
        ("袋装泡面散", 5, 1, 17),
        ("纸巾", 1, 1, 18),
        ("约吧", 1, 1, 19),
        ("茶叶", 1.5, 1, 20),
        ("充电宝", 1.5, 30, 21),
    ]
    for name, price, cap, order in products:
        conn.execute(
            "INSERT OR IGNORE INTO product_templates (product_name, unit_price, capacity, sort_order) VALUES (?,?,?,?)",
            (name, price, cap, order)
        )

    # 员工列表（从 Excel R3:V4 交班助手区域提取）
    employees = [
        ("黄河", "白班", "", 1),
        ("史红", "晚班", "", 2),
        ("刘杰", "白班", "", 3),
        ("莫健", "晚班", "", 4),
        ("贾政华", "白班", "", 5),
        ("朱晓培", "晚班", "", 6),
        ("秦佳", "白班", "", 7),
    ]
    for name, st, ph, order in employees:
        conn.execute(
            "INSERT OR IGNORE INTO employees (name, shift_type, phone, sort_order) VALUES (?,?,?,?)",
            (name, st, ph, order)
        )

    # 网费套餐（从 Excel AE5:AG21 提取）
    plans = [
        ("新会员", "特惠", 30, 1),
        ("新会员", "女神", 30, 2),
        ("新会员", "超值", 100, 3),
        ("5070显卡", "3小时", 34, 4),
        ("5070显卡", "4小时", 44, 5),
        ("5070显卡", "包天", 110, 6),
        ("网游区", "3小时", 26, 7),
        ("网游区", "4小时", 36, 8),
        ("网游区", "包天", 90, 9),
        ("网游区", "包早", 25, 10),
        ("网游区", "包夜", 45, 11),
        ("普通区", "包夜", 30, 12),
        ("普通区", "包天", 70, 13),
        ("老会员", "生日", 66, 14),
        ("电竞区5070", "通宵", 55, 15),
        ("1000网费", "送500", 1000, 16),
        ("100网费", "送20", 100, 17),
    ]
    for cat, name, price, order in plans:
        conn.execute(
            "INSERT OR IGNORE INTO fee_plans (category, plan_name, price, sort_order) VALUES (?,?,?,?)",
            (cat, name, price, order)
        )

    conn.commit()
    conn.close()

# ═══════════════════════════════════════════════════════════════════════
#  Helper: 交班逻辑
# ═══════════════════════════════════════════════════════════════════════

def get_previous_shift(shift_date, shift_type):
    """获取前一个班次（同一天白班→无前序，晚班→白班；不同天→前一天晚班）"""
    db = get_db()
    if shift_type == "晚班":
        row = db.execute(
            "SELECT * FROM shifts WHERE shift_date=? AND shift_type='白班' ORDER BY id DESC LIMIT 1",
            (shift_date,)
        ).fetchone()
    else:
        # 白班找前一天晚班
        d = datetime.strptime(shift_date, "%Y-%m-%d").date()
        from datetime import timedelta
        prev_date = (d - timedelta(days=1)).strftime("%Y-%m-%d")
        row = db.execute(
            "SELECT * FROM shifts WHERE shift_date=? AND shift_type='晚班' ORDER BY id DESC LIMIT 1",
            (prev_date,)
        ).fetchone()
    return row

def create_shift_with_template(shift_date, shift_type, operator=""):
    """新建班次，自动继承前班次库存"""
    db = get_db()
    # 创建班次记录
    cur = db.execute(
        "INSERT INTO shifts (shift_date, shift_type, operator, status) VALUES (?,?,?, 'open')",
        (shift_date, shift_type, operator)
    )
    shift_id = cur.lastrowid

    # 获取前班次的库存作为开班库存
    prev = get_previous_shift(shift_date, shift_type)
    prev_stock = {}
    if prev:
        prev_items = db.execute(
            "SELECT product_name, end_stock, unit_price, capacity FROM sales_items WHERE shift_id=?",
            (prev["id"],)
        ).fetchall()
        for pi in prev_items:
            prev_stock[pi["product_name"]] = {
                "begin_stock": pi["end_stock"],
                "unit_price": pi["unit_price"],
                "capacity": pi["capacity"],
            }

    # 从模板填充商品
    templates = db.execute(
        "SELECT * FROM product_templates ORDER BY sort_order"
    ).fetchall()
    for idx, t in enumerate(templates):
        ps = prev_stock.get(t["product_name"], {})
        begin = ps.get("begin_stock", 0)
        price = ps.get("unit_price", t["unit_price"])
        cap = ps.get("capacity", t["capacity"])
        db.execute(
            "INSERT INTO sales_items (shift_id, product_name, begin_stock, restock, sold, end_stock, unit_price, capacity, sort_order) VALUES (?,?,?,?,?,?,?,?,?)",
            (shift_id, t["product_name"], begin, 0, 0, begin, price, cap, idx)
        )

    # 开班现金 = 前班次交班现金
    cash_begin = prev["cash_end"] if prev else 0
    db.execute("UPDATE shifts SET cash_begin=? WHERE id=?", (cash_begin, shift_id))

    # 存版本快照
    save_version(shift_id, "create", operator)
    db.commit()
    return shift_id

def save_version(shift_id, action, operator=""):
    """保存班次快照到版本表"""
    db = get_db()
    shift = db.execute("SELECT * FROM shifts WHERE id=?", (shift_id,)).fetchone()
    if not shift:
        return
    snapshot = {
        "shift": dict(shift),
        "sales_items": [dict(r) for r in db.execute("SELECT * FROM sales_items WHERE shift_id=? ORDER BY sort_order", (shift_id,))],
        "expenses": [dict(r) for r in db.execute("SELECT * FROM expenses WHERE shift_id=? ORDER BY sort_order", (shift_id,))],
        "income_items": [dict(r) for r in db.execute("SELECT * FROM income_items WHERE shift_id=? ORDER BY sort_order", (shift_id,))],
        "transactions": [dict(r) for r in db.execute("SELECT * FROM transactions WHERE shift_id=? ORDER BY sort_order", (shift_id,))],
    }
    ver_num = db.execute(
        "SELECT COUNT(*) + 1 FROM shift_versions WHERE shift_id=?", (shift_id,)
    ).fetchone()[0]
    db.execute(
        "INSERT INTO shift_versions (shift_id, version_num, snapshot, action, operator) VALUES (?,?,?,?,?)",
        (shift_id, ver_num, json.dumps(snapshot, ensure_ascii=False), action, operator)
    )

def shift_to_json(shift_id):
    """获取班次完整数据"""
    db = get_db()
    shift = db.execute("SELECT * FROM shifts WHERE id=?", (shift_id,)).fetchone()
    if not shift:
        return None
    return {
        "shift": dict(shift),
        "sales_items": [dict(r) for r in db.execute("SELECT * FROM sales_items WHERE shift_id=? ORDER BY sort_order", (shift_id,))],
        "expenses": [dict(r) for r in db.execute("SELECT * FROM expenses WHERE shift_id=? ORDER BY sort_order", (shift_id,))],
        "income_items": [dict(r) for r in db.execute("SELECT * FROM income_items WHERE shift_id=? ORDER BY sort_order", (shift_id,))],
        "transactions": [dict(r) for r in db.execute("SELECT * FROM transactions WHERE shift_id=? ORDER BY sort_order", (shift_id,))],
        "versions": [dict(r) for r in db.execute(
            "SELECT id, version_num, action, operator, created_at FROM shift_versions WHERE shift_id=? ORDER BY version_num", (shift_id,)
        )],
    }

# ═══════════════════════════════════════════════════════════════════════
#  Routes — Pages
# ═══════════════════════════════════════════════════════════════════════

@app.route("/")
def index():
    return render_template("cashier.html")

# ═══════════════════════════════════════════════════════════════════════
#  Routes — API
# ═══════════════════════════════════════════════════════════════════════

@app.route("/api/employees")
def api_employees():
    """获取员工列表"""
    db = get_db()
    rows = db.execute("SELECT * FROM employees ORDER BY sort_order").fetchall()
    return jsonify({"employees": [dict(r) for r in rows]})

@app.route("/api/fee-plans")
def api_fee_plans():
    """获取网费套餐列表"""
    db = get_db()
    rows = db.execute("SELECT * FROM fee_plans ORDER BY sort_order").fetchall()
    return jsonify({"fee_plans": [dict(r) for r in rows]})

@app.route("/api/products")
def api_products():
    """获取商品模板列表"""
    db = get_db()
    rows = db.execute("SELECT * FROM product_templates ORDER BY sort_order").fetchall()
    return jsonify({"products": [dict(r) for r in rows]})

@app.route("/api/shifts/today")
def api_today():
    """获取今天的班次列表"""
    today = date.today().strftime("%Y-%m-%d")
    db = get_db()
    rows = db.execute("SELECT * FROM shifts WHERE shift_date=? ORDER BY id", (today,)).fetchall()
    return jsonify({"date": today, "shifts": [dict(r) for r in rows]})

@app.route("/api/shifts/latest")
def api_latest():
    """获取最近的班次"""
    db = get_db()
    row = db.execute("SELECT * FROM shifts ORDER BY id DESC LIMIT 1").fetchone()
    if not row:
        return jsonify({"error": "无班次"}), 404
    return jsonify(shift_to_json(row["id"]))

@app.route("/api/shifts/<int:shift_id>")
def api_get_shift(shift_id):
    """获取班次完整数据"""
    data = shift_to_json(shift_id)
    if not data:
        return jsonify({"error": "班次不存在"}), 404
    return jsonify(data)

@app.route("/api/shifts", methods=["POST"])
def api_create_shift():
    """新建班次（自动继承库存）"""
    data = request.json
    shift_date = data.get("date", date.today().strftime("%Y-%m-%d"))
    shift_type = data.get("shift_type", "白班")
    operator = data.get("operator", "")
    shift_id = create_shift_with_template(shift_date, shift_type, operator)
    return jsonify({"shift_id": shift_id, "data": shift_to_json(shift_id)})

@app.route("/api/shifts/<int:shift_id>", methods=["PUT"])
def api_update_shift(shift_id):
    """更新班次（整体保存）"""
    data = request.json
    db = get_db()
    shift = db.execute("SELECT * FROM shifts WHERE id=?", (shift_id,)).fetchone()
    if not shift:
        return jsonify({"error": "班次不存在"}), 404
    if shift["status"] == "closed" and data.get("shift", {}).get("status") != "closed":
        return jsonify({"error": "已交班的班次不能修改"}), 400

    # 更新主表
    db.execute("""
        UPDATE shifts SET
            operator=?, meituan=?, network_fee=?, network_fee_due=?,
            sales=?, expense_total=?, refund_total=?, cash_begin=?, cash_end=?,
            notes=?, status=?, updated_at=datetime('now','localtime')
        WHERE id=?
    """, (
        data["shift"].get("operator", ""),
        data["shift"].get("meituan", 0),
        data["shift"].get("network_fee", 0),
        data["shift"].get("network_fee_due", 0),
        data["shift"].get("sales", 0),
        data["shift"].get("expense_total", 0),
        data["shift"].get("refund_total", 0),
        data["shift"].get("cash_begin", 0),
        data["shift"].get("cash_end", 0),
        data["shift"].get("notes", ""),
        data["shift"].get("status", "open"),
        shift_id,
    ))

    # 更新售货明细（先删后插）
    db.execute("DELETE FROM sales_items WHERE shift_id=?", (shift_id,))
    for idx, item in enumerate(data.get("sales_items", [])):
        db.execute("""
            INSERT INTO sales_items (shift_id, product_name, begin_stock, restock, sold, end_stock, unit_price, capacity, sort_order)
            VALUES (?,?,?,?,?,?,?,?,?)
        """, (
            shift_id, item.get("product_name", ""),
            item.get("begin_stock", 0), item.get("restock", 0),
            item.get("sold", 0), item.get("end_stock", 0),
            item.get("unit_price", 0), item.get("capacity", 0), idx
        ))

    # 更新支出明细
    db.execute("DELETE FROM expenses WHERE shift_id=?", (shift_id,))
    for idx, item in enumerate(data.get("expenses", [])):
        db.execute("""
            INSERT INTO expenses (shift_id, item, amount, payment_type, sort_order)
            VALUES (?,?,?,?,?)
        """, (
            shift_id, item.get("item", ""), item.get("amount", 0),
            item.get("payment_type", ""), idx
        ))

    # 更新入账明细
    db.execute("DELETE FROM income_items WHERE shift_id=?", (shift_id,))
    for idx, item in enumerate(data.get("income_items", [])):
        db.execute("""
            INSERT INTO income_items (shift_id, item, amount, sort_order)
            VALUES (?,?,?,?)
        """, (
            shift_id, item.get("item", ""), item.get("amount", 0), idx
        ))

    # 更新交易快照
    db.execute("DELETE FROM transactions WHERE shift_id=?", (shift_id,))
    for idx, item in enumerate(data.get("transactions", [])):
        db.execute("""
            INSERT INTO transactions (shift_id, seq, product_name, coupon, actual_paid, merchant_subsidy, duration, user_phone, remark, verify_code, fee_type, sort_order)
            VALUES (?,?,?,?,?,?,?,?,?,?,?,?)
        """, (
            shift_id, item.get("seq", 0), item.get("product_name", ""),
            item.get("coupon", 0), item.get("actual_paid", 0),
            item.get("merchant_subsidy", 0), item.get("duration", ""),
            item.get("user_phone", ""), item.get("remark", ""),
            item.get("verify_code", ""), item.get("fee_type", ""), idx
        ))

    # 存版本
    save_version(shift_id, "update", data["shift"].get("operator", ""))
    db.commit()
    return jsonify({"data": shift_to_json(shift_id)})

@app.route("/api/shifts/<int:shift_id>/rollback/<int:version_id>", methods=["POST"])
def api_rollback(shift_id, version_id):
    """回滚到指定版本"""
    db = get_db()
    ver = db.execute("SELECT * FROM shift_versions WHERE id=? AND shift_id=?", (version_id, shift_id)).fetchone()
    if not ver:
        return jsonify({"error": "版本不存在"}), 404

    snapshot = json.loads(ver["snapshot"])
    shift = snapshot["shift"]

    # 恢复主表
    db.execute("""
        UPDATE shifts SET
            operator=?, meituan=?, network_fee=?, network_fee_due=?,
            sales=?, expense_total=?, refund_total=?, cash_begin=?,
            cash_end=?, notes=?, status=?, updated_at=datetime('now','localtime')
        WHERE id=?
    """, (
        shift["operator"], shift["meituan"], shift["network_fee"], shift["network_fee_due"],
        shift["sales"], shift["expense_total"], shift["refund_total"],
        shift["cash_begin"], shift["cash_end"],
        shift["notes"], shift["status"], shift_id,
    ))

    # 恢复明细（表名用白名单，列名用参数化）
    ALLOWED_TABLES = {"sales_items", "expenses", "income_items", "transactions"}
    for table, items in [
        ("sales_items", snapshot.get("sales_items", [])),
        ("expenses", snapshot.get("expenses", [])),
        ("income_items", snapshot.get("income_items", [])),
        ("transactions", snapshot.get("transactions", [])),
    ]:
        if table not in ALLOWED_TABLES:
            continue
        db.execute(f"DELETE FROM {table} WHERE shift_id=?", (shift_id,))
        for item in items:
            cols = [k for k in item.keys() if k != "id" and k != "shift_id"]
            placeholders = ", ".join(["?"] * (len(cols) + 2))
            col_names = ", ".join(["shift_id"] + cols)
            vals = [shift_id] + [item[c] for c in cols]
            db.execute(f"INSERT INTO {table} ({col_names}) VALUES ({placeholders})", vals)

    save_version(shift_id, "rollback", "")
    db.commit()
    return jsonify({"data": shift_to_json(shift_id)})

@app.route("/api/shifts/<int:shift_id>/versions")
def api_versions(shift_id):
    """获取版本列表"""
    db = get_db()
    rows = db.execute(
        "SELECT id, version_num, action, operator, created_at FROM shift_versions WHERE shift_id=? ORDER BY version_num",
        (shift_id,)
    ).fetchall()
    return jsonify({"versions": [dict(r) for r in rows]})

@app.route("/api/shifts/<int:shift_id>/versions/<int:version_id>")
def api_get_version(shift_id, version_id):
    """获取某个版本的快照"""
    db = get_db()
    ver = db.execute("SELECT * FROM shift_versions WHERE id=? AND shift_id=?", (version_id, shift_id)).fetchone()
    if not ver:
        return jsonify({"error": "版本不存在"}), 404
    return jsonify({"version": dict(ver), "snapshot": json.loads(ver["snapshot"])})

@app.route("/api/report/monthly")
def api_monthly_report():
    """月度报表"""
    year = request.args.get("year", str(date.today().year))
    month = request.args.get("month", str(date.today().month).zfill(2))
    period = f"{year}-{month}"
    db = get_db()
    rows = db.execute("""
        SELECT * FROM shifts
        WHERE shift_date LIKE ?
        ORDER BY shift_date, shift_type
    """, (f"{period}-%",)).fetchall()
    shifts = []
    for r in rows:
        s = dict(r)
        s["expenses"] = [dict(e) for e in db.execute("SELECT * FROM expenses WHERE shift_id=? ORDER BY sort_order", (r["id"],))]
        s["income_items"] = [dict(e) for e in db.execute("SELECT * FROM income_items WHERE shift_id=? ORDER BY sort_order", (r["id"],))]
        shifts.append(s)
    # 汇总
    summary = {
        "meituan": sum(s["meituan"] for s in shifts),
        "network_fee": sum(s["network_fee"] for s in shifts),
        "sales": sum(s["sales"] for s in shifts),
        "expense_total": sum(s["expense_total"] for s in shifts),
        "refund_total": sum(s["refund_total"] for s in shifts),
        "cash_balance": sum(s["cash_end"] for s in shifts),
    }
    summary["total_income"] = summary["network_fee"] + summary["sales"] + summary["refund_total"]
    summary["cash_balance"] = summary["total_income"] - summary["expense_total"] - summary["meituan"]
    return jsonify({"period": period, "summary": summary, "daily": shifts})

@app.route("/api/report/summary")
def api_summary():
    """当天/当月汇总"""
    today = date.today().strftime("%Y-%m-%d")
    db = get_db()
    today_rows = db.execute("SELECT * FROM shifts WHERE shift_date=? ORDER BY id", (today,)).fetchall()
    month_rows = db.execute("SELECT * FROM shifts WHERE shift_date LIKE ? ORDER BY id", (f"{today[:7]}-%",)).fetchall()
    def total(rows):
        return {
            "meituan": sum(r["meituan"] for r in rows),
            "network_fee": sum(r["network_fee"] for r in rows),
            "sales": sum(r["sales"] for r in rows),
            "expense_total": sum(r["expense_total"] for r in rows),
            "refund_total": sum(r["refund_total"] for r in rows),
            "shift_count": len(rows),
        }
    return jsonify({"today": total(today_rows), "month": total(month_rows)})

# ═══════════════════════════════════════════════════════════════════════
#  Main
# ═══════════════════════════════════════════════════════════════════════

if __name__ == "__main__":
    init_db()
    seed_products()
    app.run(host="0.0.0.0", port=5000, debug=False)
