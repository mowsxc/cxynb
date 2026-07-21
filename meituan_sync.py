#!/usr/bin/env python3
"""
美团订单同步辅助脚本
用法: python3 meituan_sync.py <cookie_file> <db_path> <api_url> <start_ts> <end_ts>

同步逻辑：
1. 从美团 API 拉取订单数据
2. 根据 API 返回的 description 字段判断是否为退款/撤销订单
3. 绝不靠"API 未返回"推断退款状态
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

def is_refunded_order(rec):
    """判断订单是否为退款/撤销（基于 API 返回的字段）"""
    desc = rec.get('description', '') or ''
    # 退款/撤销订单的 description 包含特定关键词
    refund_keywords = ['退款', '退费', '已退', '撤销', '撤单', '逆向']
    return any(kw in desc for kw in refund_keywords)

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

    # 切片大小：1 天（API 响应最快，约 1 秒/请求）
    MAX_SLICE_MS = 1 * 24 * 3600 * 1000
    current_start = start_ts

    while current_start < end_ts:
        current_end = min(current_start + MAX_SLICE_MS, end_ts)
        slice_records = []

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

            slice_records.extend(recs)
            if (page + 1) * 100 >= record_sum:
                break

            time.sleep(0.1)

        if errors:
            break

        all_records.extend(slice_records)
        current_start = current_end

    # 写入数据库（只使用 API 返回的数据判断状态）
    new_count = 0
    updated_count = 0
    refunded_count = 0

    if all_records and not errors:
        for rec in all_records:
            coupon = rec.get('couponValue', '')
            if not coupon:
                continue

            desc = rec.get('description', '') or ''
            ir = 1 if is_refunded_order(rec) else 0
            pi = rec.get('productInfo', '') or ''
            pt = rec.get('productTypeName', '') or ''
            sp = rec.get('salePrice', '') or ''
            dp = rec.get('discountPrice', '') or ''
            cd = rec.get('consumeDate', '') or ''
            mb = rec.get('mobile', '') or ''
            si = rec.get('consumeShopInfo', '') or ''

            existing = conn.execute(
                "SELECT is_refunded FROM orders WHERE coupon_value = ?",
                (coupon,)
            ).fetchone()

            if not existing:
                conn.execute(
                    "INSERT OR IGNORE INTO orders (coupon_value, product_info, product_type, sale_price, discount_price, consume_date, mobile, description, shop_info, is_refunded) VALUES (?,?,?,?,?,?,?,?,?,?)",
                    (coupon, pi, pt, sp, dp, cd, mb, desc, si, ir)
                )
                if conn.total_changes:
                    new_count += 1
                    if ir:
                        refunded_count += 1
            else:
                old_ir = existing[0]
                # 更新所有字段（包括退款状态）
                conn.execute(
                    "UPDATE orders SET product_info=?, product_type=?, sale_price=?, discount_price=?, consume_date=?, mobile=?, description=?, shop_info=?, is_refunded=? WHERE coupon_value=?",
                    (pi, pt, sp, dp, cd, mb, desc, si, ir, coupon)
                )
                updated_count += 1
                if ir and not old_ir:
                    refunded_count += 1

        conn.commit()

    conn.close()

    print(json.dumps({
        'new': new_count,
        'updated': updated_count,
        'refunded': refunded_count,
        'errors': errors
    }))

if __name__ == '__main__':
    main()
