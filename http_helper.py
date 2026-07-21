#!/usr/bin/env python3
"""HTTP helper for meituan-rs: sends POST request and writes JSON response to file."""
import sys
import json
import urllib.request

def main():
    if len(sys.argv) < 5:
        sys.stderr.write("Usage: http_helper.py <url> <cookie> <payload_json> <output_file>\n")
        sys.exit(1)
    
    url = sys.argv[1]
    cookie = sys.argv[2]
    payload_str = sys.argv[3]
    output_file = sys.argv[4]
    
    payload = payload_str.encode('utf-8')
    
    req = urllib.request.Request(
        url,
        data=payload,
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
        data = resp.read().decode('utf-8')
        with open(output_file, 'w') as f:
            f.write(data)
    except urllib.error.HTTPError as e:
        sys.stderr.write(f"HTTP {e.code}: {e.reason}\n")
        sys.exit(e.code)
    except Exception as e:
        sys.stderr.write(f"ERROR: {e}\n")
        sys.exit(1)

if __name__ == '__main__':
    main()
