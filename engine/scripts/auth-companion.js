#!/usr/bin/env node
/*!
 * poler-engine auth-companion v0.17.6 — интерактивное окно авторизации Google.
 * ==========================================================================
 * Локальный легковесный мост (zero-dependency: только builtin-модули Node):
 *
 *   ┌──────────────┐  spawn   ┌───────────────────────────────┐
 *   │ poler-engine │ ───────► │ auth-companion.js (этот файл) │
 *   │  --auth-ui   │          └──────────────┬────────────────┘
 *   └──────────────┘                         │ CDP (127.0.0.1:случайный порт)
 *         ▲                                  ▼
 *         │ state/exit-code    ┌───────────────────────────────┐
 *         └─────────────────── │ Chromium: ИЗОЛИРОВАННЫЙ профиль│
 *                              │ ~/.cache/poler-engine/        │
 *                              │ google-profile (userDataDir)  │
 *                              └───────────────────────────────┘
 *
 * 1. Пользователь САМ вводит логин/пароль и проходит 2FA в окне
 *    Chromium с изолированным userDataDir (никакого headless).
 * 2. После успешного входа (куки SID/HSID/SSID/APISID/SAPISID на
 *    .google.com) сессия фиксируется:
 *      • профиль уже синхронизирован — куки живут в нём (главное
 *        хранилище для --google-fetch / --nlm-*);
 *      • снапшот → ~/.config/poler-engine/google_session.json (0600).
 * 3. Гарантии безопасности (ТЗ):
 *      • No Host Snooping — ~/.config/chromium и ~/.config/google-chrome
 *        не читаются и не пишутся НИКОГДА (жёсткий guard);
 *      • Localhost Only — статус-сервер и CDP слушают строго 127.0.0.1;
 *      • Auto-termination — окно закрывается само (CDP Browser.close,
 *        куки флэшатся на диск), висячих процессов не остаётся.
 *
 * ПОЧЕМУ НЕ google_tokens.json: это строго типизированное OAuth-хранилище
 * (access/refresh token, см. src/google/oauth.rs::StoredTokens) для
 * Gmail/Drive. Браузерная сессия — другой класс креденшелов: пишем её в
 * отдельный google_session.json, не ломая OAuth-поток --google-auth.
 * OAuth-токены companion получить не может (нужен consent-флоу) — они
 * по-прежнему выдаются только через poler-engine --google-auth.
 *
 * Контракт exit-кодов (для движка):
 *   0   — авторизация зафиксирована (google_session.json записан)
 *   2   — окно закрыто до завершения входа
 *   3   — таймаут ожидания входа (POLER_AUTH_TIMEOUT_SECS, default 600)
 *   4   — preflight-ошибка (нет Node>=18/Chromium, профиль занят, snoop)
 *   130 — прервано сигналом (Ctrl+C)
 *
 * Статус-сервер (127.0.0.1, порт печатается в stdout):
 *   GET  /status  → {"state":"waiting|authorized|...","port":N,...}
 *   GET  /healthz → то же (алиас)
 *   POST /shutdown → graceful-завершение (сигнал движку/владельцу)
 *
 * Режимы:
 *   node auth-companion.js                 — штатный запуск (окно логина)
 *   node auth-companion.js --print-plan    — JSON-план без запуска браузера
 *   node auth-companion.js --self-test     — встроенные тесты (без браузера)
 *   node auth-companion.js --url <URL>     — целевой сервис вместо
 *                                            accounts.google.com
 *   node auth-companion.js --timeout <sec> — override таймаута
 *
 * Node >= 18 (без npm-зависимостей: net, http, crypto, fs, path, os).
 */

'use strict';

const net = require('net');
const http = require('http');
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');
const os = require('os');
const { spawn } = require('child_process');

const VERSION = '0.17.6';
const DEFAULT_START_URL = 'https://accounts.google.com/';
const POLL_MS = 3000;          // период опроса кук
const CONFIRM_POLLS = 2;       // подряд успешных опросов (анти-мигание)
const CDP_WAIT_MS = 30000;     // сколько ждём подъёма CDP
const WS_GUID = '258EAFA5-E914-47DA-95CA-C5AB0DC85B11';

/** Ядро браузерной сессии Google: все 5 → вход подтверждён. */
const CORE_COOKIES = ['SID', 'HSID', 'SSID', 'APISID', 'SAPISID'];

/** Exit-коды (контракт с poler-engine --auth-ui). */
const EXIT = { OK: 0, CLOSED: 2, TIMEOUT: 3, PREFLIGHT: 4, INTERRUPTED: 130 };

class CompanionError extends Error {
  constructor(message, code = EXIT.PREFLIGHT) {
    super(message);
    this.name = 'CompanionError';
    this.exitCode = code;
  }
}

// ---------------------------------------------------------------------------
// утилиты
// ---------------------------------------------------------------------------

function sleep(ms) { return new Promise((r) => setTimeout(r, ms)); }

function log(msg) { process.stdout.write(String(msg) + '\n'); }

function isoNow() { return new Date().toISOString(); }

/** Атомарная запись JSON: tmp + rename, права 0600 (POSIX). */
function atomicWriteJson600(file, obj) {
  const dir = path.dirname(file);
  fs.mkdirSync(dir, { recursive: true });
  const tmp = path.join(dir, `.${path.basename(file)}.${process.pid}.tmp`);
  fs.writeFileSync(tmp, JSON.stringify(obj, null, 2) + '\n', 'utf8');
  try { fs.chmodSync(tmp, 0o600); } catch (_) { /* не-POSIX */ }
  fs.renameSync(tmp, file);
}

/** JSONL-строка в audit-лог движка (тот же формат, что src/google/audit.rs).
 *  Best-effort: ошибка лога не ломает основной поток. Без значений кук.
 *  Файл создаётся с правами 0600 — как у движка (в логе метаданные
 *  активности аккаунта). */
function auditAppend(auditPath, action, details) {
  if (!auditPath) return;
  try {
    fs.mkdirSync(path.dirname(auditPath), { recursive: true });
    if (!fs.existsSync(auditPath)) {
      const fd = fs.openSync(auditPath, 'wx', 0o600);
      fs.closeSync(fd);
    }
    fs.appendFileSync(auditPath,
      JSON.stringify({ ts: isoNow(), action, details }) + '\n', 'utf8');
  } catch (_) { /* best-effort */ }
}

// ---------------------------------------------------------------------------
// конфигурация (чистая функция от env — тестируется без process.env)
// ---------------------------------------------------------------------------

/**
 * Разрешение путей/настроек. Приоритет как у движка (src/google/mod.rs):
 * POLER_CONFIG_DIR → ~/.config/poler-engine, POLER_GOOGLE_PROFILE →
 * ~/.cache/poler-engine/google-profile и т.д.
 */
function resolveConfig(env, opts = {}) {
  const home = env.HOME || os.homedir() || '.';
  const configDir = env.POLER_CONFIG_DIR || path.join(home, '.config', 'poler-engine');
  const profileDir = env.POLER_GOOGLE_PROFILE ||
    path.join(home, '.cache', 'poler-engine', 'google-profile');
  const auditEnv = (env.POLER_AUDIT_LOG || '').trim();
  const timeoutEnv = parseInt(env.POLER_AUTH_TIMEOUT_SECS || '', 10);
  return {
    home,
    configDir,
    profileDir,
    tokensPath: env.POLER_GOOGLE_TOKENS || path.join(configDir, 'google_tokens.json'),
    sessionPath: env.POLER_GOOGLE_SESSION || path.join(configDir, 'google_session.json'),
    statePath: path.join(configDir, 'auth-companion.state.json'),
    auditPath: auditEnv.toLowerCase() === 'off' ? null
      : (auditEnv || path.join(configDir, 'audit.log')),
    statusPort: parseInt(env.POLER_AUTH_COMPANION_PORT || '0', 10) || 0, // 0 = эфемерный
    googleCdpPort: parseInt(env.POLER_GOOGLE_CDP_PORT || '9223', 10) || 9223,
    timeoutSecs: (Number.isFinite(timeoutEnv) && timeoutEnv >= 30)
      ? Math.min(timeoutEnv, 7200)
      : (opts.timeoutSecs && opts.timeoutSecs >= 30 ? Math.min(opts.timeoutSecs, 7200) : 600),
    startUrl: opts.url || DEFAULT_START_URL,
    chromeBin: env.POLER_CHROME_BIN || null,
    noSandbox: (env.POLER_CHROME_NO_SANDBOX || '').toLowerCase() === '1',
  };
}

// ---------------------------------------------------------------------------
// guard: No Host Snooping
// ---------------------------------------------------------------------------

/** Основные профили браузеров хоста — под абсолютным запретом. */
function hostBrowserProfiles(home) {
  return [
    path.join(home, '.config', 'chromium'),
    path.join(home, '.config', 'google-chrome'),
    path.join(home, '.config', 'chromium-browser'),
    path.join(home, '.mozilla'),
  ];
}

/**
 * Твёрдый отказ, если изолированный профиль указывает на (или внутрь)
 * основного браузера хоста. Кидает CompanionError(EXIT.PREFLIGHT).
 */
function assertNoHostSnooping(cfg) {
  const norm = (p) => path.resolve(String(p));
  const prof = norm(cfg.profileDir);
  for (const host of hostBrowserProfiles(cfg.home)) {
    const h = norm(host);
    if (prof === h || prof.startsWith(h + path.sep)) {
      throw new CompanionError(
        `No Host Snooping: профиль движка (${prof}) указывает на основной ` +
        `профиль браузера хоста (${h}). Откажись от POLER_GOOGLE_PROFILE ` +
        `или укажи каталог вне пользовательских браузерных профилей.`, EXIT.PREFLIGHT);
    }
  }
}

// ---------------------------------------------------------------------------
// поиск браузера (полный Chromium с окном; headless-shell не подходит)
// ---------------------------------------------------------------------------

function isHeadlessShell(bin) {
  return /headless-shell/i.test(path.basename(String(bin)));
}

function findBrowserBin(env = process.env) {
  if (env.POLER_CHROME_BIN) {
    if (fs.existsSync(env.POLER_CHROME_BIN)) return env.POLER_CHROME_BIN;
    return null; // явно задан, но не существует — не молчим, а падаем
  }
  const names = ['chromium', 'chromium-browser', 'google-chrome',
    'google-chrome-stable', 'chrome'];
  const dirs = (env.PATH || '').split(':');
  for (const n of names) {
    for (const d of dirs) {
      const p = path.join(d, n);
      if (fs.existsSync(p)) return p;
    }
  }
  for (const p of ['/usr/bin/chromium', '/usr/bin/chromium-browser',
    '/usr/bin/google-chrome', '/snap/bin/chromium']) {
    if (fs.existsSync(p)) return p;
  }
  return null;
}

/** Аргументы запуска окна логина. ВАЖНО: БЕЗ --disable-web-security —
 *  окно логина должно оставаться штатно-защищённым браузером. */
function browserLaunchArgs(cfg, cdpPort) {
  const args = [
    `--user-data-dir=${path.resolve(cfg.profileDir)}`,
    `--remote-debugging-port=${cdpPort}`,
    '--remote-debugging-address=127.0.0.1', // DevTools строго на loopback
    '--no-first-run',
    '--no-default-browser-check',
  ];
  // --no-sandbox только по явному opt-in (контейнеры без user-namespace).
  if (cfg.noSandbox) args.push('--no-sandbox');
  args.push(cfg.startUrl);
  return args;
}

/** Свободный порт на 127.0.0.1 (listen(0) → закрываем → отдаём). */
function freePort() {
  return new Promise((resolve, reject) => {
    const s = net.createServer();
    s.once('error', reject);
    s.listen(0, '127.0.0.1', () => {
      const port = s.address().port;
      s.close(() => resolve(port));
    });
  });
}

/** HTTP GET к CDP-эндпоинту (например /json/version), ответ — JSON. */
function cdpHttp(port, p, timeoutMs = 5000) {
  return new Promise((resolve, reject) => {
    const req = http.get({ host: '127.0.0.1', port, path: p, timeout: timeoutMs },
      (res) => {
        let b = '';
        res.on('data', (c) => { b += c; });
        res.on('end', () => {
          try { resolve(JSON.parse(b)); }
          catch (e) { reject(new Error(`CDP ${p}: некорректный JSON (${e.message})`)); }
        });
      });
    req.once('timeout', () => req.destroy(new Error(`CDP ${p}: таймаут`)));
    req.once('error', reject);
  });
}

/** Жив ли google-браузер движка на его стандартном CDP-порту? */
async function cdpPing(port, timeoutMs = 1500) {
  try { await cdpHttp(port, '/json/version', timeoutMs); return true; }
  catch (_) { return false; }
}

// ---------------------------------------------------------------------------
// MiniWs: минимальный RFC 6455 WebSocket-клиент (CDP) без зависимостей
// ---------------------------------------------------------------------------

/** Поддерживает: handshake, текстовые фреймы, фрагментацию, ping/pong,
 *  close-хендшейк, длины 7/16/64 бит, маскирование клиентских фреймов. */
class MiniWs {
  constructor(sock) {
    this.sock = sock;
    this.buf = Buffer.alloc(0);
    this.fragments = [];
    this.fragOpcode = 0;
    this.onMessage = null; // (text) => void
    this.onClose = null;   // () => void
    this._closeEmitted = false;
  }

  static connect(port, wsPath, timeoutMs = 10000) {
    return new Promise((resolve, reject) => {
      const key = crypto.randomBytes(16).toString('base64');
      const sock = net.connect({ host: '127.0.0.1', port });
      let ws = null;
      let handshaked = false;
      let buf = Buffer.alloc(0);
      const timer = setTimeout(() => {
        sock.destroy();
        reject(new Error(`WS connect ${wsPath}: таймаут ${timeoutMs}мс`));
      }, timeoutMs);

      sock.once('error', (e) => {
        clearTimeout(timer);
        reject(e);
      });
      sock.once('connect', () => {
        sock.write(
          `GET ${wsPath} HTTP/1.1\r\n` +
          `Host: 127.0.0.1:${port}\r\n` +
          `Upgrade: websocket\r\n` +
          `Connection: Upgrade\r\n` +
          `Sec-WebSocket-Key: ${key}\r\n` +
          `Sec-WebSocket-Version: 13\r\n\r\n`);
      });
      sock.on('data', (d) => {
        if (!handshaked) {
          buf = Buffer.concat([buf, d]);
          const i = buf.indexOf('\r\n\r\n');
          if (i === -1) return;
          const head = buf.slice(0, i).toString('latin1');
          if (!/^HTTP\/1\.1 101/.test(head)) {
            clearTimeout(timer);
            sock.destroy();
            reject(new Error(`WS handshake отклонён: ${head.split('\r\n')[0]}`));
            return;
          }
          handshaked = true;
          clearTimeout(timer);
          ws = new MiniWs(sock);
          const rest = buf.slice(i + 4);
          resolve(ws);
          if (rest.length) ws._feed(rest);
          return;
        }
        if (ws) ws._feed(d);
      });
      sock.once('close', () => {
        if (ws) ws._shutdown();
        else if (!handshaked) {
          clearTimeout(timer);
          reject(new Error('WS: сокет закрыт до handshake'));
        }
      });
    });
  }

  get isOpen() {
    return !this._closeEmitted && this.sock && !this.sock.destroyed;
  }

  send(text) { this._sendFrame(0x1, Buffer.from(String(text), 'utf8')); }

  /** Отправка без ожидания ответа (для Browser.close). */
  notify(method, params = {}) {
    this.send(JSON.stringify({ id: 0, method, params }));
  }

  close() {
    try {
      if (this.isOpen) this._sendFrame(0x8, Buffer.alloc(0));
      this.sock.end();
    } catch (_) { /* уже закрыт */ }
    this._shutdown();
  }

  // ----- внутреннее -----

  _feed(d) {
    this.buf = this.buf.length ? Buffer.concat([this.buf, d]) : d;
    for (;;) {
      const frame = this._parseFrame();
      if (!frame) break;
      this._handleFrame(frame);
      if (this._closeEmitted) break;
    }
  }

  _parseFrame() {
    const b = this.buf;
    if (b.length < 2) return null;
    const fin = (b[0] & 0x80) !== 0;
    const opcode = b[0] & 0x0f;
    const masked = (b[1] & 0x80) !== 0;
    let len = b[1] & 0x7f;
    let off = 2;
    if (len === 126) {
      if (b.length < off + 2) return null;
      len = b.readUInt16BE(off); off += 2;
    } else if (len === 127) {
      if (b.length < off + 8) return null;
      const hi = b.readUInt32BE(off);
      const lo = b.readUInt32BE(off + 4);
      len = hi * 4294967296 + lo;
      off += 8;
    }
    let maskKey = null;
    if (masked) {
      if (b.length < off + 4) return null;
      maskKey = b.slice(off, off + 4); off += 4;
    }
    if (b.length < off + len) return null;
    let payload = b.slice(off, off + len);
    this.buf = b.slice(off + len);
    if (maskKey) {
      const out = Buffer.allocUnsafe(payload.length);
      for (let i = 0; i < payload.length; i++) out[i] = payload[i] ^ maskKey[i & 3];
      payload = out;
    }
    return { fin, opcode, payload };
  }

  _handleFrame(f) {
    switch (f.opcode) {
      case 0x0: // continuation
        if (this.fragOpcode) {
          this.fragments.push(f.payload);
          if (f.fin) {
            const full = Buffer.concat(this.fragments);
            const op = this.fragOpcode;
            this.fragments = [];
            this.fragOpcode = 0;
            this._deliver(op, full);
          }
        }
        break;
      case 0x1:
      case 0x2:
        if (f.fin) this._deliver(f.opcode, f.payload);
        else { this.fragments = [f.payload]; this.fragOpcode = f.opcode; }
        break;
      case 0x8: // close
        try { if (this.isOpen) this._sendFrame(0x8, f.payload); } catch (_) {}
        this._shutdown();
        break;
      case 0x9: // ping → pong
        this._sendFrame(0xA, f.payload);
        break;
      case 0xA: break; // pong — игнор
      default: break;
    }
  }

  _deliver(opcode, payload) {
    if (opcode === 0x1 && this.onMessage) {
      try { this.onMessage(payload.toString('utf8')); } catch (_) {}
    }
  }

  _sendFrame(opcode, payload) {
    if (!this.sock || this.sock.destroyed) return;
    const mask = crypto.randomBytes(4);
    const len = payload.length;
    let header;
    if (len < 126) {
      header = Buffer.alloc(2);
      header[1] = 0x80 | len;
    } else if (len < 65536) {
      header = Buffer.alloc(4);
      header[1] = 0x80 | 126;
      header.writeUInt16BE(len, 2);
    } else {
      header = Buffer.alloc(10);
      header[1] = 0x80 | 127;
      header.writeUInt32BE(Math.floor(len / 4294967296), 2);
      header.writeUInt32BE(len >>> 0, 6);
    }
    header[0] = 0x80 | opcode;
    const masked = Buffer.allocUnsafe(payload.length);
    for (let i = 0; i < payload.length; i++) masked[i] = payload[i] ^ mask[i & 3];
    this.sock.write(Buffer.concat([header, mask, masked]));
  }

  _shutdown() {
    if (this._closeEmitted) return;
    this._closeEmitted = true;
    if (this.onClose) {
      try { this.onClose(); } catch (_) {}
    }
  }
}

// ---------------------------------------------------------------------------
// CdpClient: вызовы методов CDP поверх MiniWs
// ---------------------------------------------------------------------------

class CdpClient {
  constructor(ws) {
    this.ws = ws;
    this.nextId = 1;
    this.pending = new Map();
    ws.onMessage = (text) => {
      let m;
      try { m = JSON.parse(text); } catch (_) { return; }
      if (m && m.id && this.pending.has(m.id)) {
        const { resolve, reject } = this.pending.get(m.id);
        this.pending.delete(m.id);
        if (m.error) reject(new Error(m.error.message || 'CDP error'));
        else resolve(m.result);
      }
    };
    ws.onClose = () => {
      for (const { reject } of this.pending.values()) {
        reject(new Error('CDP: соединение закрыто'));
      }
      this.pending.clear();
    };
  }

  call(method, params = {}, timeoutMs = 15000) {
    return new Promise((resolve, reject) => {
      if (!this.ws.isOpen) { reject(new Error('CDP: WS не открыт')); return; }
      const id = ++this.nextId;
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`CDP таймаут: ${method}`));
      }, timeoutMs);
      this.pending.set(id, {
        resolve: (v) => { clearTimeout(timer); resolve(v); },
        reject: (e) => { clearTimeout(timer); reject(e); },
      });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }

  /** Все куки браузера (Storage.getCookies, фолбэк Network.getAllCookies). */
  async getAllCookies() {
    try {
      const r = await this.call('Storage.getCookies');
      if (Array.isArray(r.cookies)) return r.cookies;
    } catch (_) { /* deprecated/недоступен → фолбэк */ }
    const r = await this.call('Network.getAllCookies');
    return Array.isArray(r.cookies) ? r.cookies : [];
  }

  /** Graceful-закрытие браузера (куки флэшатся на диск). Fire-and-forget:
   *  ответ может не прийти — браузер умирает раньше. */
  closeBrowser() {
    this.ws.notify('Browser.close');
  }
}

// ---------------------------------------------------------------------------
// логика сессии Google (чистые функции — покрыты self-test)
// ---------------------------------------------------------------------------

/** Домен относится к Google? ('.google.com', 'accounts.google.com', …) */
function isGoogleDomain(domain) {
  const d = String(domain || '').replace(/^\./, '').toLowerCase();
  return d === 'google.com' || d.endsWith('.google.com');
}

/** Куки только google-доменов (NotebookLM входит: *.notebooklm.google.com). */
function googleSessionCookies(cookies) {
  return (cookies || []).filter((c) => isGoogleDomain(c.domain));
}

/**
 * Ядро сессии: все CORE_COOKIES непустые?
 * @returns {{ok: boolean, missing: string[]}} — missing = чего не хватает.
 */
function coreCookieStatus(gCookies) {
  const have = new Set(
    (gCookies || []).filter((c) => String(c.value || '').length > 0).map((c) => c.name));
  const missing = CORE_COOKIES.filter((n) => !have.has(n));
  return { ok: missing.length === 0, missing };
}

/** Снапшот сессии для google_session.json (включая значения кук —
 *  файл секретный, 0600; печатать значения наружу нельзя). */
function buildSnapshot(cookies, source) {
  const g = googleSessionCookies(cookies);
  const st = coreCookieStatus(g);
  return {
    captured_at: isoNow(),
    core_ok: st.ok,
    missing_core: st.missing,
    cookie_names: g.map((c) => c.name),
    cookies: g.map((c) => ({
      name: String(c.name || ''),
      value: String(c.value || ''),
      domain: String(c.domain || ''),
      path: String(c.path || '/'),
      expires: typeof c.expires === 'number' ? c.expires : -1,
      secure: !!c.secure,
      httpOnly: !!c.httpOnly,
      sameSite: c.sameSite || null,
    })),
    source: source || `auth-companion ${VERSION}`,
  };
}

// ---------------------------------------------------------------------------
// статус-сервер (СТРОГО 127.0.0.1) — сигнал готовности для движка
// ---------------------------------------------------------------------------

/**
 * Поднимает localhost-сервер состояния.
 * @returns {Promise<{server, port, requestShutdown: function}>}
 */
function startStatusServer(cfg, stateRef) {
  return new Promise((resolve, reject) => {
    let shutdownCb = null;
    const server = http.createServer((req, res) => {
      const u = new URL(req.url, 'http://127.0.0.1');
      if (req.method === 'GET' && (u.pathname === '/status' || u.pathname === '/healthz')) {
        res.writeHead(200, {
          'Content-Type': 'application/json; charset=utf-8',
          'Cache-Control': 'no-store',
        });
        res.end(JSON.stringify({
          companion: 'poler-engine auth-companion',
          version: VERSION,
          ...stateRef,
        }));
        return;
      }
      if (req.method === 'POST' && u.pathname === '/shutdown') {
        res.writeHead(200, { 'Content-Type': 'application/json; charset=utf-8' });
        res.end('{"ok":true,"action":"shutdown"}');
        if (shutdownCb) setImmediate(shutdownCb);
        return;
      }
      res.writeHead(404, { 'Content-Type': 'application/json; charset=utf-8' });
      res.end('{"error":"not found"}');
    });
    server.once('error', reject);
    // КЛЮЧЕВАЯ гарантия: слушаем только loopback, никогда 0.0.0.0.
    server.listen(cfg.statusPort, '127.0.0.1', () => {
      const port = server.address().port;
      const addr = server.address().address; // '127.0.0.1'
      if (addr !== '127.0.0.1') {
        server.close();
        reject(new Error(`статус-сервер забиндился на ${addr} — запрещено, только 127.0.0.1`));
        return;
      }
      stateRef.port = port;
      resolve({
        server,
        port,
        requestShutdown: (cb) => { shutdownCb = cb; },
      });
    });
  });
}

/** Запись transient-состояния (для движка и наблюдаемости; без секретов). */
function setState(stateRef, cfg, state, detail) {
  stateRef.state = state;
  stateRef.detail = String(detail || '');
  stateRef.ts = isoNow();
  try { atomicWriteJson600(cfg.statePath, { ...stateRef }); } catch (_) { /* best-effort */ }
}

// ---------------------------------------------------------------------------
// главный поток
// ---------------------------------------------------------------------------

function waitExit(child, ms) {
  if (child.exitCode !== null || child.signalCode) return Promise.resolve(true);
  return new Promise((resolve) => {
    const t = setTimeout(() => resolve(false), ms);
    child.once('exit', () => { clearTimeout(t); resolve(true); });
  });
}

async function ensureCdp(cdpPort, clientRef) {
  if (clientRef.cdp && clientRef.cdp.ws.isOpen) return clientRef.cdp;
  if (clientRef.cdp) { try { clientRef.cdp.ws.close(); } catch (_) {} clientRef.cdp = null; }
  const ver = await cdpHttp(cdpPort, '/json/version');
  const wsUrl = String(ver.webSocketDebuggerUrl || '');
  if (!wsUrl) throw new Error('CDP: webSocketDebuggerUrl отсутствует');
  const m = /ws:\/\/[^/]+(\/.*)$/.exec(wsUrl);
  const wsPath = m ? m[1] : '/devtools/browser';
  const ws = await MiniWs.connect(cdpPort, wsPath);
  clientRef.cdp = new CdpClient(ws);
  return clientRef.cdp;
}

/**
 * Штатный запуск: preflight → окно → поллинг кук → снапшот → закрытие.
 * Возвращает exit-код (см. EXIT).
 */
async function runCompanion(cfg) {
  // --- preflight ---
  const nodeMajor = parseInt((process.versions.node || '0').split('.')[0], 10);
  if (nodeMajor < 18) {
    throw new CompanionError(`нужен Node >= 18 (сейчас ${process.versions.node})`, EXIT.PREFLIGHT);
  }
  assertNoHostSnooping(cfg);
  const bin = findBrowserBin();
  if (!bin) {
    throw new CompanionError(
      'Chromium не найден. Установи полный браузер (окно для ручного входа) или задай ' +
      'POLER_CHROME_BIN=/путь/к/chromium. Headless-shell не подходит: нет окна для 2FA.',
      EXIT.PREFLIGHT);
  }
  if (isHeadlessShell(bin)) {
    throw new CompanionError(
      `${bin} — headless-shell, у него нет окна для ручного входа. ` +
      'Укажи POLER_CHROME_BIN= на полный Chromium.', EXIT.PREFLIGHT);
  }
  if (await cdpPing(cfg.googleCdpPort)) {
    throw new CompanionError(
      `порт CDP ${cfg.googleCdpPort} занят — похоже, уже открыт google-браузер poler-engine ` +
      '(--google-browse или зависший --google-fetch). Профиль один: закрой то окно и повтори.',
      EXIT.PREFLIGHT);
  }
  fs.mkdirSync(cfg.profileDir, { recursive: true });
  fs.mkdirSync(cfg.configDir, { recursive: true });
  // Повисшие Singleton-замки после крашей прибираем (как ensure_google_browser).
  for (const n of ['SingletonLock', 'SingletonCookie', 'SingletonSocket']) {
    try { fs.rmSync(path.join(cfg.profileDir, n), { force: true }); } catch (_) {}
  }

  const cdpPort = await freePort();
  const stateRef = {
    state: 'starting', detail: '', ts: isoNow(), port: 0, cookie_count: 0,
    cdp_port: cdpPort, pid: process.pid,
  };
  const clientRef = { cdp: null };
  const status = await startStatusServer(cfg, stateRef);
  let shutdownRequested = false;
  status.requestShutdown(() => { shutdownRequested = true; });

  const finish = async (code, state, detail) => {
    setState(stateRef, cfg, state, detail);
    auditAppend(cfg.auditPath, 'security.auth_companion',
      `state=${state} exit=${code} cookies=${stateRef.cookie_count}`);
    try { status.server.close(); } catch (_) {}
    if (clientRef.cdp) { try { clientRef.cdp.ws.close(); } catch (_) {} }
    return code;
  };

  // --- запуск окна логина ---
  const args = browserLaunchArgs(cfg, cdpPort);
  log(`auth-companion v${VERSION}: окно входа Google`);
  log(`  браузер:   ${bin}`);
  log(`  профиль:   ${cfg.profileDir} (изолирован; хост-браузеры не трогаем)`);
  log(`  таймаут:   ${cfg.timeoutSecs} c`);
  log(`  статус:    http://127.0.0.1:${status.port}/status`);
  log('');
  log('Введи логин/пароль и пройди 2FA в открывшемся окне — САМ, своими руками.');
  log('Пароль остаётся между тобой и Google: companion читает только итоговые');
  log('куки сессии (SID/HSID/SSID/APISID/SAPISID), не формы ввода.');

  const child = spawn(bin, args, { stdio: ['ignore', 'ignore', 'ignore'] });
  let exited = false;
  child.once('exit', () => { exited = true; });

  // Ctrl+C: прибираем окно и выходим без висячих процессов.
  const onSignal = () => {
    try { if (child.exitCode === null) child.kill('SIGTERM'); } catch (_) {}
    setTimeout(() => { try { child.kill('SIGKILL'); } catch (_) {} }, 3000);
    process.exitCode = EXIT.INTERRUPTED;
    shutdownRequested = true;
  };
  process.once('SIGINT', onSignal);
  process.once('SIGTERM', onSignal);

  // --- ждём CDP (или ранний выход браузера: профиль занят другим окном) ---
  let cdpUp = false;
  for (let i = 0; i < Math.ceil(CDP_WAIT_MS / 500) && !exited; i++) {
    await sleep(500);
    if (await cdpPing(cdpPort, 1000)) { cdpUp = true; break; }
  }
  if (!cdpUp) {
    try { if (child.exitCode === null) child.kill('SIGTERM'); } catch (_) {}
    if (exited) {
      return await finish(EXIT.CLOSED, 'closed',
        'браузер вышел сразу — профиль, вероятно, занят другим окном Chrome/Chromium');
    }
    return await finish(EXIT.PREFLIGHT, 'error', `CDP не поднялся на 127.0.0.1:${cdpPort} за ${CDP_WAIT_MS / 1000} c`);
  }
  setState(stateRef, cfg, 'waiting', `окно открыто: ${cfg.startUrl}`);

  // --- поллинг кук до полного ядра сессии ---
  const deadline = Date.now() + cfg.timeoutSecs * 1000;
  let confirmed = 0;
  let snapshot = null;
  while (!exited && !shutdownRequested && Date.now() < deadline) {
    await sleep(POLL_MS);
    if (exited || shutdownRequested) break;
    try {
      const cdp = await ensureCdp(cdpPort, clientRef);
      const cookies = await cdp.getAllCookies();
      const g = googleSessionCookies(cookies);
      stateRef.cookie_count = g.length;
      const st = coreCookieStatus(g);
      if (st.ok) {
        confirmed += 1;
        if (confirmed >= CONFIRM_POLLS) { snapshot = buildSnapshot(cookies); break; }
      } else {
        confirmed = 0;
      }
    } catch (_) { /* CDP мигнул — переподключимся на следующем такте */ }
  }

  process.removeListener('SIGINT', onSignal);
  process.removeListener('SIGTERM', onSignal);

  if (snapshot) {
    // --- успех: снапшот (0600) + graceful-закрытие окна ---
    atomicWriteJson600(cfg.sessionPath, snapshot);
    log('');
    log(`✓ Сессия Google захвачена: ${snapshot.cookies.length} кук google-доменов.`);
    log(`  Снапшот:  ${cfg.sessionPath} (0600)`);
    log(`  Профиль:  ${cfg.profileDir} — куки сохранены, окно закрываю.`);
    try { (clientRef.cdp || await ensureCdp(cdpPort, clientRef)).closeBrowser(); } catch (_) {}
    if (!await waitExit(child, 5000)) {
      try { child.kill('SIGTERM'); } catch (_) {}
      if (!await waitExit(child, 3000)) { try { child.kill('SIGKILL'); } catch (_) {} }
    }
    return await finish(EXIT.OK, 'authorized',
      `cookies=${snapshot.cookies.length} core=ok`);
  }

  // --- без логина: закрываем окно за собой в любом случае ---
  try { (clientRef.cdp || (await ensureCdp(cdpPort, clientRef).catch(() => null)) || { closeBrowser: () => {} }).closeBrowser(); } catch (_) {}
  if (!await waitExit(child, 5000)) {
    try { child.kill('SIGTERM'); } catch (_) {}
    if (!await waitExit(child, 3000)) { try { child.kill('SIGKILL'); } catch (_) {} }
  }
  if (exited) {
    return await finish(EXIT.CLOSED, 'closed', 'окно закрыто до завершения входа');
  }
  if (shutdownRequested) {
    return await finish(EXIT.INTERRUPTED, 'interrupted', 'прервано (сигнал или /shutdown)');
  }
  return await finish(EXIT.TIMEOUT, 'timeout', `вход не подтверждён за ${cfg.timeoutSecs} c`);
}

// ---------------------------------------------------------------------------
// --print-plan: JSON-план запуска (без браузера; для интеграций и отладки)
// ---------------------------------------------------------------------------

function buildPlan(cfg) {
  const bin = findBrowserBin();
  return {
    companion: 'poler-engine auth-companion',
    version: VERSION,
    node: process.versions.node,
    config_dir: cfg.configDir,
    profile_dir: cfg.profileDir,
    session_path: cfg.sessionPath,
    state_path: cfg.statePath,
    audit_path: cfg.auditPath,
    oauth_tokens_path: cfg.tokensPath,
    oauth_tokens_touched: false, // никогда: OAuth — территория --google-auth
    start_url: cfg.startUrl,
    timeout_secs: cfg.timeoutSecs,
    status_bind: '127.0.0.1',
    core_cookies: CORE_COOKIES,
    exit_codes: EXIT,
    browser: bin ? {
      bin,
      headless_shell: isHeadlessShell(bin),
      args: browserLaunchArgs(cfg, 0),
    } : null,
  };
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

function printUsage() {
  log([
    `poler-engine auth-companion v${VERSION} — интерактивное окно авторизации Google`,
    '',
    'Использование:',
    '  node auth-companion.js                  запустить окно входа',
    '  node auth-companion.js --url <URL>      целевой сервис (default: accounts.google.com)',
    '  node auth-companion.js --timeout <sec>  таймаут ожидания входа (default: 600)',
    '  node auth-companion.js --print-plan     JSON-план без запуска браузера',
    '  node auth-companion.js --self-test      встроенные тесты',
    '',
    'Env: POLER_CONFIG_DIR, POLER_GOOGLE_PROFILE, POLER_GOOGLE_SESSION,',
    '     POLER_CHROME_BIN, POLER_CHROME_NO_SANDBOX=1, POLER_AUTH_TIMEOUT_SECS,',
    '     POLER_AUTH_COMPANION_PORT, POLER_AUDIT_LOG.',
  ].join('\n'));
}

function parseArgs(argv) {
  const opts = { url: null, timeoutSecs: null };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--self-test') opts.selfTest = true;
    else if (a === '--print-plan') opts.printPlan = true;
    else if (a === '--url') opts.url = argv[++i];
    else if (a === '--timeout') opts.timeoutSecs = parseInt(argv[++i], 10);
    else if (a === '--help' || a === '-h') { printUsage(); process.exit(0); }
    else {
      process.stderr.write(`auth-companion: неизвестный аргумент: ${a}\n`);
      process.exit(EXIT.PREFLIGHT);
    }
  }
  return opts;
}

async function main() {
  const opts = parseArgs(process.argv);
  if (opts.selfTest) return runSelfTest();
  const cfg = resolveConfig(process.env, opts);
  if (opts.printPlan) {
    log(JSON.stringify(buildPlan(cfg), null, 2));
    return EXIT.OK;
  }
  return runCompanion(cfg);
}

// ---------------------------------------------------------------------------
// --self-test: встроенные тесты без браузера (песочница/CI)
// ---------------------------------------------------------------------------
const tests = [];
function test(name, fn) { tests.push([name, fn]); }
function assert(cond, msg) { if (!cond) throw new Error(msg || 'assertion failed'); }
function assertEq(actual, expected, msg) {
  if (actual !== expected) {
    throw new Error(`${msg || 'assertEq'}: ожидалось ${JSON.stringify(expected)}, получено ${JSON.stringify(actual)}`);
  }
}

// --- тестовый WS-эхо-сервер (server-сторона RFC 6455) ---
function startWsEchoServer() {
  return new Promise((resolve) => {
    const srv = net.createServer((sock) => {
      let handshaked = false;
      let buf = Buffer.alloc(0);
      const pongs = [];
      sock.on('data', (d) => {
        buf = Buffer.concat([buf, d]);
        if (!handshaked) {
          const i = buf.indexOf('\r\n\r\n');
          if (i === -1) return;
          const head = buf.slice(0, i).toString('latin1');
          const m = /Sec-WebSocket-Key: (.+)\r\n/.exec(head);
          if (!m) { sock.destroy(); return; }
          const accept = crypto.createHash('sha1')
            .update(m[1].trim() + WS_GUID).digest('base64');
          sock.write('HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n' +
            `Connection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
          buf = buf.slice(i + 4);
          handshaked = true;
        }
        // разбор (замаскированных) клиентских фреймов
        for (;;) {
          if (buf.length < 2) break;
          const opcode = buf[0] & 0x0f;
          const masked = (buf[1] & 0x80) !== 0;
          let len = buf[1] & 0x7f;
          let off = 2;
          if (len === 126) { if (buf.length < 4) break; len = buf.readUInt16BE(2); off = 4; }
          else if (len === 127) {
            if (buf.length < 10) break;
            len = buf.readUInt32BE(2) * 4294967296 + buf.readUInt32BE(6); off = 10;
          }
          let maskKey = null;
          if (masked) { if (buf.length < off + 4) break; maskKey = buf.slice(off, off + 4); off += 4; }
          if (buf.length < off + len) break;
          let payload = buf.slice(off, off + len);
          buf = buf.slice(off + len);
          if (maskKey) {
            const out = Buffer.allocUnsafe(payload.length);
            for (let k = 0; k < payload.length; k++) out[k] = payload[k] ^ maskKey[k & 3];
            payload = out;
          }
          if (opcode === 0x1) { // text → echo
            const text = payload.toString('utf8');
            if (text === 'CMD:FRAG') {
              // три фрагмента: проверка реассемблирования на клиенте
              srvSend(sock, 0x1, Buffer.from('FRAG', 'utf8'), false);
              srvSend(sock, 0x0, Buffer.from('MENT', 'utf8'), false);
              srvSend(sock, 0x0, Buffer.from('ED-OK', 'utf8'), true);
            } else if (text === 'CMD:PING') {
              srvSend(sock, 0x9, Buffer.from('hb', 'utf8'), true);
            } else {
              srvSend(sock, 0x1, payload, true);
            }
          } else if (opcode === 0x8) { // close → эхо close и закрыть
            srvSend(sock, 0x8, payload, true);
            sock.end();
          } else if (opcode === 0xA) { // pong от клиента
            pongs.push(payload.toString('utf8'));
            srvSend(sock, 0x1, Buffer.from('PONG:' + pongs.join(','), 'utf8'), true);
          }
        }
      });
      sock.on('error', () => {});
    });
    srv.listen(0, '127.0.0.1', () => resolve({ srv, port: srv.address().port }));
  });
}

/** Отправка фрейма от сервера (без маски — так требует RFC для сервера). */
function srvSend(sock, opcode, payload, fin) {
  const len = payload.length;
  let header;
  if (len < 126) { header = Buffer.alloc(2); header[1] = len; }
  else if (len < 65536) { header = Buffer.alloc(4); header[1] = 126; header.writeUInt16BE(len, 2); }
  else {
    header = Buffer.alloc(10);
    header[1] = 127;
    header.writeUInt32BE(Math.floor(len / 4294967296), 2);
    header.writeUInt32BE(len % 4294967296, 6);
  }
  header[0] = (fin ? 0x80 : 0x00) | opcode;
  sock.write(Buffer.concat([header, payload]));
}

// --- регистрация тестов ---

test('coreCookieStatus: полное ядро → ok', () => {
  const cookies = CORE_COOKIES.map((n) => ({ name: n, value: 'x', domain: '.google.com' }));
  const st = coreCookieStatus(cookies);
  assert(st.ok, 'ядро полное — ok');
  assertEq(st.missing.length, 0, 'missing пуст');
});

test('coreCookieStatus: пустые значения не считаются', () => {
  const cookies = CORE_COOKIES.map((n) => ({ name: n, value: '', domain: '.google.com' }));
  const st = coreCookieStatus(cookies);
  assert(!st.ok, 'пустые значения → не ok');
  assertEq(st.missing.join(','), CORE_COOKIES.join(','), 'все в missing');
});

test('coreCookieStatus: частичное ядро → список отсутствующих', () => {
  const cookies = [{ name: 'SID', value: 'a', domain: '.google.com' },
    { name: 'HSID', value: 'b', domain: '.google.com' }];
  const st = coreCookieStatus(cookies);
  assert(!st.ok, 'не ok');
  assertEq(st.missing.join(','), 'SSID,APISID,SAPISID', 'missing список');
});

test('isGoogleDomain: границы', () => {
  assert(isGoogleDomain('.google.com'), '.google.com');
  assert(isGoogleDomain('accounts.google.com'), 'accounts.google.com');
  assert(isGoogleDomain('notebooklm.google.com'), 'notebooklm.google.com');
  assert(isGoogleDomain('GOOGLE.COM'), 'регистронезависимость');
  assert(!isGoogleDomain('google.com.evil.example'), 'evil-суффикс');
  assert(!isGoogleDomain('notgoogle.com'), 'чужой домен');
  assert(!isGoogleDomain(''), 'пустой');
});

test('googleSessionCookies: только google-домены', () => {
  const out = googleSessionCookies([
    { name: 'SID', value: '1', domain: '.google.com' },
    { name: 'evilsid', value: '2', domain: 'google.com.evil.example' },
    { name: 'other', value: '3', domain: 'example.com' },
  ]);
  assertEq(out.length, 1, 'одна гугл-кука');
  assertEq(out[0].name, 'SID', 'имя');
});

test('buildSnapshot: форма и полнота', () => {
  const cookies = [
    ...CORE_COOKIES.map((n) => ({
      name: n, value: 'v-' + n, domain: '.google.com', path: '/',
      expires: 1234567890, secure: true, httpOnly: true, sameSite: 'Lax',
    })),
    { name: 'x', value: 'y', domain: 'example.com' }, // не google — отбрасывается
  ];
  const snap = buildSnapshot(cookies, 'test');
  assert(snap.core_ok, 'core_ok');
  assertEq(snap.cookies.length, 5, '5 гугл-кук');
  assertEq(snap.cookie_names.join(','), CORE_COOKIES.join(','), 'имена');
  assertEq(snap.cookies[0].httpOnly, true, 'httpOnly camelCase');
  assert(snap.captured_at.endsWith('Z'), 'ISO-метка');
  assertEq(snap.source, 'test', 'source');
});

test('assertNoHostSnooping: отказ на профиле хоста', () => {
  const home = '/tmp/fake-home-selftest';
  for (const bad of [path.join(home, '.config', 'chromium'),
    path.join(home, '.config', 'chromium', 'Default'),
    path.join(home, '.config', 'google-chrome')]) {
    let threw = false;
    try { assertNoHostSnooping({ home, profileDir: bad }); }
    catch (e) { threw = true; assert(/No Host Snooping/.test(e.message), 'текст guard'); }
    assert(threw, 'guard сработал: ' + bad);
  }
});

test('assertNoHostSnooping: изолированный профиль — разрешён', () => {
  const home = '/tmp/fake-home-selftest';
  assertNoHostSnooping({
    home,
    profileDir: path.join(home, '.cache', 'poler-engine', 'google-profile'),
  });
});

test('atomicWriteJson600: атомарность, права 0600, без tmp-мусора', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'poler-ac-'));
  const file = path.join(dir, 'google_session.json');
  atomicWriteJson600(file, { a: 1, b: 'текст' });
  const st = fs.statSync(file);
  assertEq(st.mode & 0o777, 0o600, 'права 0600');
  assertEq(JSON.parse(fs.readFileSync(file, 'utf8')).b, 'текст', 'контент');
  const leftovers = fs.readdirSync(dir).filter((n) => n.includes('.tmp'));
  assertEq(leftovers.length, 0, 'tmp переименован');
  fs.rmSync(dir, { recursive: true, force: true });
});

test('auditAppend: JSONL, две строки, формат движка', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'poler-audit-'));
  const f = path.join(dir, 'audit.log');
  auditAppend(f, 'security.auth_companion', 'state=test cookies=5');
  auditAppend(f, 'security.auth_companion', 'state=test2 cookies=6');
  const lines = fs.readFileSync(f, 'utf8').trim().split('\n');
  assertEq(lines.length, 2, 'две строки');
  assertEq(fs.statSync(f).mode & 0o777, 0o600, 'права 0600 при создании');
  const rec = JSON.parse(lines[0]);
  assertEq(rec.action, 'security.auth_companion', 'action');
  assert(/\d{4}-\d{2}-\d{2}T/.test(rec.ts), 'ts ISO-8601');
  assertEq(rec.details, 'state=test cookies=5', 'details');
  fs.rmSync(dir, { recursive: true, force: true });
});

test('auditAppend: null-путь (POLER_AUDIT_LOG=off) — тихо', () => {
  auditAppend(null, 'x', 'y'); // не бросает
});

test('resolveConfig: env-приоритеты как у движка', () => {
  const env = {
    HOME: '/tmp/fake-home',
    POLER_CONFIG_DIR: '/tmp/cfg',
    POLER_GOOGLE_PROFILE: '/tmp/prof',
    POLER_GOOGLE_SESSION: '/tmp/sess.json',
    POLER_AUTH_TIMEOUT_SECS: '120',
    POLER_AUDIT_LOG: 'off',
  };
  const cfg = resolveConfig(env);
  assertEq(cfg.configDir, '/tmp/cfg', 'configDir');
  assertEq(cfg.profileDir, '/tmp/prof', 'profileDir');
  assertEq(cfg.sessionPath, '/tmp/sess.json', 'sessionPath');
  assertEq(cfg.timeoutSecs, 120, 'timeout из env');
  assertEq(cfg.auditPath, null, 'audit off');
  assertEq(cfg.tokensPath, '/tmp/cfg/google_tokens.json', 'tokens default');
  const cfg2 = resolveConfig({ HOME: '/tmp/fake-home' }, { url: 'https://notebooklm.google.com/' });
  assertEq(cfg2.profileDir, '/tmp/fake-home/.cache/poler-engine/google-profile', 'default profile');
  assertEq(cfg2.timeoutSecs, 600, 'default timeout');
  assertEq(cfg2.startUrl, 'https://notebooklm.google.com/', 'startUrl из opts');
});

test('browserLaunchArgs: изоляция и loopback, без disable-web-security', () => {
  const args = browserLaunchArgs({
    profileDir: '/tmp/prof', startUrl: 'https://accounts.google.com/', noSandbox: false,
  }, 42424);
  assert(args.includes('--user-data-dir=/tmp/prof'), 'изолированный userDataDir');
  assert(args.includes('--remote-debugging-port=42424'), 'cdp порт');
  assert(args.includes('--remote-debugging-address=127.0.0.1'), 'loopback devtools');
  assert(args.includes('https://accounts.google.com/'), 'стартовый URL');
  assert(!args.includes('--disable-web-security'), 'окно логина без ослабления безопасности');
  assert(!args.includes('--no-sandbox'), 'sandbox включён по умолчанию');
  const args2 = browserLaunchArgs({
    profileDir: '/tmp/prof', startUrl: 'https://accounts.google.com/', noSandbox: true,
  }, 1);
  assert(args2.includes('--no-sandbox'), 'opt-in no-sandbox');
});

test('статус-сервер: 127.0.0.1 only, /status, /shutdown', async () => {
  const cfg = { statusPort: 0 };
  const stateRef = { state: 'waiting', cookie_count: 0 };
  const status = await startStatusServer(cfg, stateRef);
  assertEq(status.server.address().address, '127.0.0.1', 'bind только loopback');
  const body = await new Promise((resolve, reject) => {
    http.get({ host: '127.0.0.1', port: status.port, path: '/status' }, (res) => {
      let b = '';
      res.on('data', (c) => { b += c; });
      res.on('end', () => resolve(JSON.parse(b)));
    }).once('error', reject);
  });
  assertEq(body.state, 'waiting', 'state в ответе');
  assertEq(body.version, VERSION, 'version в ответе');
  let shutdownHit = false;
  status.requestShutdown(() => { shutdownHit = true; });
  await new Promise((resolve, reject) => {
    const req = http.request({
      host: '127.0.0.1', port: status.port, path: '/shutdown', method: 'POST',
    }, (res) => { res.resume(); res.once('end', resolve); });
    req.once('error', reject);
    req.end();
  });
  await sleep(50);
  assert(shutdownHit, 'POST /shutdown вызвал callback');
  await new Promise((r) => status.server.close(r));
});

test('MiniWs: echo, большой фрейм (64-bit len), фрагментация, ping/pong, close', async () => {
  const echo = await startWsEchoServer();
  const ws = await MiniWs.connect(echo.port, '/devtools/test');
  const received = [];
  ws.onMessage = (t) => received.push(t);
  // маленькое сообщение
  ws.send('hello');
  // большое (выйдет за 64 КБ → 64-bit длина в обе стороны)
  const big = 'A'.repeat(200000);
  ws.send(big);
  await sleep(300);
  assertEq(received[0], 'hello', 'эхо малого');
  assertEq(received[1], big, 'эхо большого (64-bit length + маскирование)');
  // фрагментированная отправка сервера
  ws.send('CMD:FRAG');
  await sleep(200);
  assertEq(received[2], 'FRAGMENTED-OK', 'реассемблирование фрагментов');
  // ping → клиент обязан ответить pong
  ws.send('CMD:PING');
  await sleep(200);
  assert(received.some((t) => t === 'PONG:hb'), 'pong отправлен клиентом');
  // close-хендшейк
  let closed = false;
  ws.onClose = () => { closed = true; };
  ws.close();
  await sleep(200);
  assert(closed, 'onClose вызван');
  echo.srv.close();
});

test('MiniWs: handshake на несуществующий порт → reject', async () => {
  const port = await freePort(); // свободный, но никто не слушает
  let threw = false;
  try { await MiniWs.connect(port, '/devtools/x', 2000); }
  catch (_) { threw = true; }
  assert(threw, 'ECONNREFUSED → reject');
});

test('CdpClient: request/response по id + reject на мёртвом WS', async () => {
  const echo = await startWsEchoServer();
  const ws = await MiniWs.connect(echo.port, '/devtools/test');
  const cdp = new CdpClient(ws);
  // Эхо возвращает наш JSON как есть → id совпадает → промис резолвится
  // (m.result отсутствует → undefined). Это проверяет весь путь:
  // send → маскированный фрейм → echo → парсинг → resolve по id.
  const r = await cdp.call('Storage.getCookies', {}, 2000);
  assertEq(r, undefined, 'echo-ответ сматчился по id и резолвился');
  // Мёртвый WS → немедленный reject без таймаута.
  ws.close();
  await sleep(100);
  let rejected = false;
  try { await cdp.call('Storage.getCookies', {}, 500); }
  catch (e) { rejected = /WS не открыт/.test(e.message); }
  assert(rejected, 'мёртвый WS → reject «WS не открыт»');
  echo.srv.close();
});

test('freePort: возвращает пригодный порт', async () => {
  const p = await freePort();
  assert(Number.isInteger(p) && p > 0 && p < 65536, 'корректный порт');
  // порт реально свободен: bind снова проходит
  await new Promise((resolve, reject) => {
    const s = net.createServer();
    s.once('error', reject);
    s.listen(p, '127.0.0.1', () => s.close(resolve));
  });
});

test('EXIT-контракт совпадает с движком (src/google/auth_ui.rs)', () => {
  assertEq(EXIT.OK, 0, 'OK');
  assertEq(EXIT.CLOSED, 2, 'CLOSED');
  assertEq(EXIT.TIMEOUT, 3, 'TIMEOUT');
  assertEq(EXIT.PREFLIGHT, 4, 'PREFLIGHT');
  assertEq(EXIT.INTERRUPTED, 130, 'INTERRUPTED');
  assertEq(CORE_COOKIES.join(','), 'SID,HSID,SSID,APISID,SAPISID', 'ядро сессии');
});

async function runSelfTest() {
  let failed = 0;
  for (const [name, fn] of tests) {
    try {
      await fn();
      log(`ok - ${name}`);
    } catch (e) {
      failed += 1;
      log(`NOT OK - ${name}: ${e.message}`);
    }
  }
  log(failed ? `FAILED: ${failed} из ${tests.length}` : `ALL ${tests.length} PASS`);
  return failed ? 1 : 0;
}

// --- entry-point строго в конце файла: к этому моменту инициализированы
//     и основной код, и self-test секция ---
if (require.main === module) {
  main()
    .then((code) => process.exit(code))
    .catch((e) => {
      const code = (e && e.exitCode) || EXIT.PREFLIGHT;
      process.stderr.write(`auth-companion: ${e && e.message ? e.message : e}\n`);
      process.exit(code);
    });
}

