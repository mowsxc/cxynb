#!/usr/bin/env python3
"""
美团订单同步辅助脚本
用法: python3 meituan_sync.py <cookie_file> <db_path> <api_url> <start_ts> <end_ts>
"""
import sys
import json
import time
import sqlite3
import urllib.request

def http_post(url, cookie, payload):
    data = json.dumps(payload).encode('utf-8')
    req = urllib.request.Request(
        url, data=data,
        headers={
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
            'Content-Type': 'application/json',
            'Origin': 'https://e.dianping.com',
            'Referer': 'https://e.dianping.com/app/np-mer-voucher-web-static/records',
            'Cookie': cookie
        },
        method='POST'
    )
    try:
        resp = urllib.request.urlopen(req, timeout=10)
        return json.loads(resp.read().decode('utf-8'))
    except urllib.error.HTTPError as e:
        return {'error': f'HTTP {e.code}: {e.reason}'}
    except Exception as e:
        return {'error': str(e)}

def main():
    if len(sys.argv) < 6:
        print(json.dumps({'error': '参数不足'}))
        sys.exit(1)
    
    cookie_file = sys.argv[1]
    db_path = sys.argv[2]
    api_url = sys.argv[3]
    start_ts = int(sys.argv[4])
    end_ts = int(sys.argv[5])
    
    # 读取 Cookie
    try:
        with open(cookie_file, 'r') as f:
            cookies = json.load(f)
        cookie_str = '; '.join(f'{c["name"]}={c["value"]}' for c in cookies)
    except Exception as e:
        print(json.dumps({'error': f'Cookie 读取失败: {e}'}))
        sys.exit(1)
    
    # 连接数据库
    conn = sqlite3.connect(db_path)
    
    all_records = []
    errors = []
    
    MAX_SLICE_MS = 3 * 24 * 3600 * 1000
    current_start = start_ts
    
    while current_start < end_ts:
        current_end = min(current_start + MAX_SLICE_MS, end_ts)
        
        for page in range(50):
            payload = {
                "dealGroupIds": "", "bussinessType": 0, "shopIds": "0", "productTabNum": 1,
                "offset": page * 100, "limit": 100,
                "beginDate": current_start, "endDate": current_end,
                "subTabNum": None, "isConsumeMedical": False
            }
            
            result = http_post(api_url, cookie_str, payload)
            
            if 'error' in result:
                errors.append(f"page {page}: {result['error']}")
                break
            
            d = result.get('data')
            if not d:
                break
            
            record_sum = d.get('recordSum', 0)
            if page == 0 and record_sum == 0:
                break
            
            recs = d.get('couponRecordDetails', [])
            if not recs:
                break
            
            all_records.extend(recs)
            if (page + 1) * 100 >= record_sum:
                break
            
            time.sleep(0.3)
        
        if errors:
            break
        current_start = current_end
    
    # 写入数据库
    new_count = 0
    updated_count = 0
    
    if all_records and not errors:
        for rec in all_records:
            coupon = rec.get('couponValue', '')
            if not coupon:
                continue
            
            desc = rec.get('description', '')
            ir = 1 if any(kw in desc for kw in ['退款', '退费', '已退', '撤销', '撤单']) else 0
            pi = rec.get('productInfo', '')
            pt = rec.get('productTypeName', '')
            sp = rec.get('salePrice', '')
            dp = rec.get('discountPrice', '')
            cd = rec.get('consumeDate', '')
            mb = rec.get('mobile', '')
            de = rec.get('description', '')
            si = rec.get('consumeShopInfo', '')
            # 兼容多种字段名
            if not pi: pi = rec.get('product_info', '')
            if not pt: pt = rec.get('product_type_name', '')
            if not sp: sp = rec.get('sale_price', '')
            if not dp: dp = rec.get('discount_price', '')
            if not cd: cd = rec.get('consume_date', '')
            if not si: si = rec.get('consume_shop_info', '')
            
            existing = conn.execute(
                "SELECT product_info, product_type, sale_price, discount_price, consume_date, mobile, description, shop_info, is_refunded FROM orders WHERE coupon_value = ?",
                (coupon,)
            ).fetchone()
            
            if not existing:
                conn.execute(
                    "INSERT OR IGNORE INTO orders (coupon_value, product_info, product_type, sale_price, discount_price, consume_date, mobile, description, shop_info, is_refunded) VALUES (?,?,?,?,?,?,?,?,?,?)",
                    (coupon, pi, pt, sp, dp, cd, mb, de, si, ir)
                )
                if conn.total_changes:
                    new_count += 1
            else:
                old_pi, old_pt, old_sp, old_dp, old_cd, old_mb, old_de, old_si, old_ir = existing
                if (pi, pt, sp, dp, cd, mb, de, si, ir) != (old_pi, old_pt, old_sp, old_dp, old_cd, old_mb, old_de, old_si, old_ir):
                    conn.execute(
                        "UPDATE orders SET product_info=?, product_type=?, sale_price=?, discount_price=?, consume_date=?, mobile=?, description=?, shop_info=?, is_refunded=? WHERE coupon_value=?",
                        (pi, pt, sp, dp, cd, mb, de, si, ir, coupon)
                    )
                    updated_count += 1
        
        conn.commit()
    
    # 检查撤销订单
    now_ts = int(time.time() * 1000)
    start_dt = time.strftime('%Y-%m-%d %H:%M:%S', time.localtime(start_ts / 1000))
    rows = conn.execute(
        "SELECT coupon_value, consume_date FROM orders WHERE consume_date >= ? AND is_refunded = 0",
        (start_dt,)
    ).fetchall()
    
    api_coupon_set = {r.get('couponValue', '') for r in all_records}
    revoked_count = 0
    
    for coupon, consume_date in rows:
        if coupon not in api_coupon_set:
            try:
                order_ts = int(time.mktime(time.strptime(consume_date, '%Y-%m-%d %H:%M:%S')) * 1000)
                if now_ts - order_ts > 120000:
                    conn.execute(
                        "UPDATE orders SET is_refunded=1, description='[已撤销]' WHERE coupon_value=?",
                        (coupon,)
                    )
                    revoked_count += 1
            except:
                pass
    
    conn.commit()
    conn.close()
    
    print(json.dumps({
        'new': new_count,
        'updated': updated_count + revoked_count,
        'errors': errors
    }))

if __name__ == '__main__':
    main()
