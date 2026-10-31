const { spawn } = require('child_process');
const http = require('http');
const net = require('net');
const crypto = require('crypto');
const fs = require('fs');
const os = require('os');
const path = require('path');

const CHROME = '/usr/bin/chromium';
const PROFILE = path.join(os.homedir(), '.cache', 'poler-engine', 'google-profile');
const CDP_PORT = 47890;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function getJsonPort(port, p) {
  return new Promise((res, reject) => {
    const req = http.get({ host: '127.0.0.1', port, path: p, timeout: 3000 }, (r) => {
      let b = '';
      r.on('data', (c) => (b += c));
      r.on('end', () => {
        try { res(JSON.parse(b)); } catch (e) { reject(e); }
      });
    });
    req.on('error', reject);
    req.on('timeout', () => req.destroy());
  });
}

class MiniWs {
  constructor(sock) {
    this.sock = sock;
    this.buf = Buffer.alloc(0);
    this.onMessage = null;
    sock.on('data', (d) => this._feed(d));
  }
  static connect(port, wsPath) {
    return new Promise((resolve, reject) => {
      const key = crypto.randomBytes(16).toString('base64');
      const sock = net.connect({ host: '127.0.0.1', port });
      let handshaked = false;
      sock.once('error', reject);
      sock.once('connect', () => {
        sock.write(
          `GET ${wsPath} HTTP/1.1\r\nHost: 127.0.0.1:${port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: ${key}\r\nSec-WebSocket-Version: 13\r\n\r\n`
        );
      });
      sock.on('data', function onFirst(d) {
        if (!handshaked) {
          const s = d.toString('latin1');
          if (!/^HTTP\/1\.1 101/.test(s)) {
            sock.destroy();
            return reject(new Error('WS handshake failed'));
          }
          handshaked = true;
          sock.removeListener('data', onFirst);
          const i = d.indexOf('\r\n\r\n');
          const ws = new MiniWs(sock);
          if (i !== -1 && i + 4 < d.length) ws._feed(d.slice(i + 4));
          resolve(ws);
        }
      });
    });
  }
  send(text) {
    const p = Buffer.from(text, 'utf8');
    const mask = crypto.randomBytes(4);
    const m = Buffer.allocUnsafe(p.length);
    for (let i = 0; i < p.length; i++) m[i] = p[i] ^ mask[i & 3];
    let h;
    if (p.length < 126) {
      h = Buffer.alloc(2);
      h[1] = 0x80 | p.length;
    } else if (p.length < 65536) {
      h = Buffer.alloc(4);
      h[1] = 0x80 | 126;
      h.writeUInt16BE(p.length, 2);
    } else {
      h = Buffer.alloc(10);
      h[1] = 0x80 | 127;
      h.writeUInt32BE(Math.floor(p.length / 4294967296), 2);
      h.writeUInt32BE(p.length >>> 0, 6);
    }
    h[0] = 0x81;
    this.sock.write(Buffer.concat([h, mask, m]));
  }
  _feed(d) {
    this.buf = Buffer.concat([this.buf, d]);
    for (;;) {
      const b = this.buf;
      if (b.length < 2) break;
      const masked = (b[1] & 0x80) !== 0;
      let len = b[1] & 0x7f;
      let off = 2;
      if (len === 126) {
        if (b.length < 4) break;
        len = b.readUInt16BE(2);
        off = 4;
      } else if (len === 127) {
        if (b.length < 10) break;
        len = b.readUInt32BE(2) * 4294967296 + b.readUInt32BE(6);
        off = 10;
      }
      if (masked) {
        if (b.length < off + 4) break;
        off += 4;
      }
      if (b.length < off + len) break;
      const payload = b.slice(off, off + len).toString('utf8');
      this.buf = b.slice(off + len);
      if (this.onMessage) this.onMessage(payload);
    }
  }
}

class CdpClient {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pending = new Map();
    ws.onMessage = (t) => {
      try {
        const m = JSON.parse(t);
        if (m.id && this.pending.has(m.id)) {
          const p = this.pending.get(m.id);
          this.pending.delete(m.id);
          if (m.error) p.reject(new Error(m.error.message));
          else p.resolve(m.result);
        }
      } catch (_) {}
    };
  }
  call(method, params = {}, timeoutMs = 15000) {
    return new Promise((resolve, reject) => {
      const id = ++this.id;
      this.pending.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ id, method, params }));
      setTimeout(() => {
        if (this.pending.has(id)) {
          this.pending.delete(id);
          reject(new Error(method + ' timeout'));
        }
      }, timeoutMs);
    });
  }
}

(async () => {
  console.log('[1/4] Запуск /usr/bin/chromium с профилем движка...');
  const child = spawn(CHROME, [
    `--user-data-dir=${PROFILE}`,
    `--remote-debugging-port=${CDP_PORT}`,
    '--remote-debugging-address=127.0.0.1',
    '--no-first-run',
    '--no-default-browser-check',
    '--no-sandbox',
    '--headless=new',
    '--disable-gpu',
    'about:blank'
  ], { stdio: 'ignore' });

  for (let i = 0; i < 40; i++) {
    await sleep(300);
    try {
      await getJsonPort(CDP_PORT, '/json/version');
      break;
    } catch (_) {}
  }

  console.log('[2/4] Подключение к Chromium...');
  const list = await getJsonPort(CDP_PORT, '/json/list');
  const page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl);
  const m = /ws:\/\/[^/]+(\/.*)$/.exec(page.webSocketDebuggerUrl);
  const ws = await MiniWs.connect(CDP_PORT, m[1]);
  const cdp = new CdpClient(ws);
  await cdp.call('Page.enable');
  await cdp.call('Runtime.enable');

  console.log('[3/4] Открытие Gmail с поиском from:creativefabrica.com...');
  await cdp.call('Page.navigate', { url: 'https://mail.google.com/mail/u/0/#search/from%3Acreativefabrica.com' });
  await sleep(10000);

  const res = await cdp.call('Runtime.evaluate', {
    expression: 'document.title + " | " + (document.body ? document.body.innerText.slice(0, 400).replace(/\\s+/g, " ") : "")',
    returnByValue: true
  });
  console.log('ЭКРАН GMAIL:\n', res.result.value);

  child.kill('SIGKILL');
  console.log('[4/4] Готово.');
})().catch(e => {
  console.error('Ошибка:', e.message);
  process.exit(1);
});
