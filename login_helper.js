#!/usr/bin/env node
// 美团登录辅助脚本：通过 CDP 获取真实 QR 码，轮询登录状态，提取 Cookie
const { spawn } = require('child_process');
const WebSocket = require('ws');
const http = require('http');
const fs = require('fs');
const path = require('path');

const CDP_PORT = 9223;
const STATUS_FILE = '/workspace/login_status.json';
const QRCODE_FILE = '/workspace/login_qrcode.png';
const COOKIE_FILE = '/workspace/meituan_cookies.json';
const LOGIN_URL = 'https://e.dianping.com';

function setStatus(status, extra = {}) {
  const data = { status, time: new Date().toISOString(), ...extra };
  fs.writeFileSync(STATUS_FILE, JSON.stringify(data, null, 2));
  console.log(`[${data.time}] status: ${status}`, extra);
}

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

function httpGet(url) {
  return new Promise((resolve, reject) => {
    http.get(url, (res) => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => resolve(data));
    }).on('error', reject);
  });
}

async function getCdpWsUrl() {
  for (let i = 0; i < 30; i++) {
    try {
      const data = await httpGet(`http://127.0.0.1:${CDP_PORT}/json`);
      const pages = JSON.parse(data);
      const page = pages.find(p => p.type === 'page');
      if (page && page.webSocketDebuggerUrl) {
        return page.webSocketDebuggerUrl;
      }
    } catch (e) {}
    await sleep(1000);
  }
  throw new Error('CDP 连接超时');
}

function sendCdp(ws, id, method, params = {}) {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`CDP ${method} timeout`)), 30000);
    const handler = (data) => {
      try {
        const msg = JSON.parse(data.toString());
        if (msg.id === id) {
          clearTimeout(timeout);
          ws.removeListener('message', handler);
          resolve(msg);
        }
      } catch (e) {}
    };
    ws.on('message', handler);
    ws.send(JSON.stringify({ id, method, params }));
  });
}

async function extractQrCode(ws) {
  // 尝试多种方式获取 QR 码

  // 方式1: 页面截图，尝试裁剪 QR 码区域
  // 先找到 QR 码元素
  const qrSelectors = [
    'img[src*="qrcode"]',
    'img[src*="qr"]',
    'canvas',
    '.qrcode img',
    '.login-qrcode img',
    '#qrcode img',
    'img[alt*="二维码"]',
  ];

  let qrBounds = null;
  for (const sel of qrSelectors) {
    try {
      const result = await sendCdp(ws, 100, 'Runtime.evaluate', {
        expression: `(function(){
          var el = document.querySelector('${sel}');
          if (!el) return null;
          var r = el.getBoundingClientRect();
          return { x: r.x, y: r.y, w: r.width, h: r.height, tag: el.tagName, src: el.src || '', visible: r.width > 50 && r.height > 50 };
        })()`
      });
      const val = result.result?.result?.value;
      if (val && val.visible) {
        qrBounds = val;
        console.log(`找到 QR 码元素: ${val.tag} ${val.w}x${val.h}`, val.src ? '(has src)' : '');
        break;
      }
    } catch (e) {}
  }

  if (!qrBounds) {
    // 方式2: 尝试让页面截取可视区域的截图
    console.log('未找到 QR 码元素，截取页面中央区域');
    throw new Error('QR_NOT_FOUND');
  }

  // 截取页面截图并裁剪 QR 码区域
  const viewport = await sendCdp(ws, 101, 'Page.getLayoutMetrics');
  console.log('viewport:', JSON.stringify(viewport.result?.cssLayoutViewport));

  return qrBounds;
}

async function takeQrScreenshot(ws, qrBounds) {
  const result = await sendCdp(ws, 102, 'Page.captureScreenshot', {
    format: 'png',
    clip: {
      x: Math.max(0, qrBounds.x),
      y: Math.max(0, qrBounds.y),
      width: qrBounds.w,
      height: qrBounds.h,
      scale: 1
    }
  });

  const base64 = result.result?.data;
  if (base64) {
    fs.writeFileSync(QRCODE_FILE, Buffer.from(base64, 'base64'));
    console.log('QR 码已保存');
    return true;
  }
  return false;
}

async function checkLoggedIn(ws) {
  try {
    const result = await sendCdp(ws, 200, 'Runtime.evaluate', {
      expression: `(function(){
        var u = window.location.href;
        return { url: u, isLoggedIn: !u.includes('login') && !u.includes('passport') && !u.includes('unitivelogin') };
      })()`
    });
    const val = result.result?.result?.value;
    return val?.isLoggedIn === true;
  } catch (e) {
    return false;
  }
}

async function extractCookies(ws) {
  const result = await sendCdp(ws, 300, 'Network.getAllCookies');
  const cookies = result.result?.cookies || [];

  const dianpingCookies = cookies.filter(c =>
    c.domain && (c.domain.includes('dianping') || c.domain.includes('meituan'))
  );

  // 保存为后端需要的 JSON 格式
  const formatted = dianpingCookies.map(c => ({
    name: c.name,
    value: c.value,
    domain: c.domain,
    path: c.path || '/',
  }));

  fs.writeFileSync(COOKIE_FILE, JSON.stringify(formatted, null, 2));
  console.log(`已提取 ${formatted.length} 个 Cookie`);
  return formatted;
}

async function main() {
  setStatus('starting');

  // 清理旧状态
  try { fs.unlinkSync(QRCODE_FILE); } catch (e) {}
  try { fs.unlinkSync(COOKIE_FILE); } catch (e) {}

  // 启动 Chromium
  console.log('启动 Chromium headless...');
  const browser = spawn('chromium', [
    `--remote-debugging-port=${CDP_PORT}`,
    '--headless=new',
    '--no-sandbox',
    '--disable-gpu',
    '--disable-dev-shm-usage',
    '--window-size=1280,800',
    LOGIN_URL,
  ], {
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  browser.stderr.on('data', d => console.log('[chromium]', d.toString().trim()));
  browser.on('exit', code => console.log('Chromium exited with', code));

  let ws;
  try {
    // 获取 CDP WebSocket
    setStatus('connecting');
    const wsUrl = await getCdpWsUrl();
    ws = new WebSocket(wsUrl);
    await new Promise((resolve) => ws.on('open', resolve));
    console.log('CDP 已连接');

    // 等待页面加载
    setStatus('loading_page');
    await sleep(5000);

    // 启用必要的 CDP domains
    await sendCdp(ws, 1, 'Page.enable');
    await sendCdp(ws, 2, 'Network.enable');
    await sendCdp(ws, 3, 'Runtime.enable');

    // 等待更久让 SPA 渲染完毕
    setStatus('waiting_render');
    console.log('等待页面渲染...');
    await sleep(8000);

    // 尝试获取 QR 码（带重试）
    let qrCaptured = false;
    for (let attempt = 0; attempt < 10 && !qrCaptured; attempt++) {
      try {
        setStatus('extracting_qr', { attempt: attempt + 1 });
        console.log(`尝试获取 QR 码 (第 ${attempt + 1} 次)...`);

        // 先检查是否已经登录（可能已有 cookie）
        const loggedIn = await checkLoggedIn(ws);
        if (loggedIn) {
          setStatus('already_logged_in');
          console.log('已登录状态，无需 QR 码');
          await extractCookies(ws);
          browser.kill();
          return;
        }

        // 获取页面整体截图作为 QR 码
        // 先用 Runtime.evaluate 查找可能的 QR 码位置
        const findResult = await sendCdp(ws, 10, 'Runtime.evaluate', {
          returnByValue: true,
          expression: `(function(){
            var imgs = document.querySelectorAll('img');
            for (var i = 0; i < imgs.length; i++) {
              var r = imgs[i].getBoundingClientRect();
              var src = imgs[i].src || '';
              if (r.width > 100 && r.height > 100 && (src.includes('qr') || src.includes('qrcode') || src.includes('token'))) {
                return {x:r.x, y:r.y, w:r.width, h:r.height, tag:'IMG', src:src};
              }
            }
            var canvases = document.querySelectorAll('canvas');
            for (var i = 0; i < canvases.length; i++) {
              var r = canvases[i].getBoundingClientRect();
              if (r.width > 100 && r.height > 100) {
                return {x:r.x, y:r.y, w:r.width, h:r.height, tag:'CANVAS'};
              }
            }
            return null;
          })()`
        });

        let bounds = findResult.result?.result?.value;
        if (!bounds) {
          await sleep(3000);
          continue;
        }

        console.log('QR 区域:', JSON.stringify(bounds));

        // 截取该区域
        try {
          const screenshot = await sendCdp(ws, 11, 'Page.captureScreenshot', {
            format: 'png',
            clip: {
              x: bounds.x,
              y: bounds.y,
              width: bounds.w,
              height: bounds.h,
              scale: 1
            }
          });
          if (screenshot.result?.data) {
            fs.writeFileSync(QRCODE_FILE, Buffer.from(screenshot.result.data, 'base64'));
            console.log('QR 码截图已保存');
            qrCaptured = true;
          }
        } catch (e) {
          console.log('截图失败:', e.message);
        }
      } catch (e) {
        console.log('尝试失败:', e.message);
        await sleep(3000);
      }
    }

    if (!qrCaptured) {
      // 降级：截取页面整体
      console.log('QR 码获取失败，截取整体页面');
      try {
        const fullShot = await sendCdp(ws, 12, 'Page.captureScreenshot', { format: 'png' });
        if (fullShot.result?.data) {
          fs.writeFileSync(QRCODE_FILE, Buffer.from(fullShot.result.data, 'base64'));
          qrCaptured = true;
        }
      } catch (e) {}
    }

    if (!qrCaptured) {
      setStatus('error', { error: '无法获取登录二维码，请使用 Cookie 粘贴方式登录' });
      browser.kill();
      return;
    }

    // QR 码已就绪，等待用户扫码登录
    setStatus('waiting_scan', { qr_ready: true });
    console.log('等待用户扫码登录...');

    // 轮询登录状态
    for (let i = 0; i < 180; i++) {  // 最多等待 6 分钟
      await sleep(2000);
      try {
        // 检查页面 URL 是否变化
        const urlResult = await sendCdp(ws, 20 + i, 'Runtime.evaluate', {
          expression: 'window.location.href'
        });
        const currentUrl = urlResult.result?.result?.value || '';
        console.log(`轮询 [${i + 1}]:`, currentUrl.substring(0, 80));

        if (currentUrl && !currentUrl.includes('passport') && !currentUrl.includes('login') && currentUrl.includes('dianping')) {
          setStatus('logged_in');
          console.log('登录成功！');

          // 额外等待页面完全加载
          await sleep(3000);

          // 提取 Cookie
          await extractCookies(ws);
          setStatus('done', { cookies_saved: true });
          browser.kill();
          return;
        }
      } catch (e) {
        console.log('轮询错误:', e.message);
      }

      if (i % 15 === 14) {
        setStatus('waiting_scan', { qr_ready: true, elapsed_seconds: (i + 1) * 2 });
      }
    }

    // 超时
    setStatus('expired');
    browser.kill();
  } catch (e) {
    console.error('脚本错误:', e);
    setStatus('error', { error: e.message });
    browser.kill();
  }
}

main().catch(e => {
  console.error('致命错误:', e);
  setStatus('error', { error: e.message });
  process.exit(1);
});
