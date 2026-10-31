// browser-service — интерактивный удалённый браузер для превью POLER
// Управление: клиент шлёт события мыши/клавиатуры, сервер гонит их в
// Chromium (Playwright, постоянный профиль — куки выживают перезапуски)
// и стримит JPEG-кадры обратно. Плюс одноразовая привязка Google Диска:
// rclone authorize стартует ЗДЕСЬ, страница входа открывается в удалённом
// браузере, OAuth-редирект на 127.0.0.1:53682 попадает куда надо,
// токен автоматически пишется в ~/.config/rclone/rclone.conf ([gdrive]).

import { createServer } from 'http'
import { Server } from 'socket.io'
import { chromium } from 'playwright'
import { spawn, spawnSync, type ChildProcess } from 'child_process'
import { existsSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'fs'
import * as path from 'path'
import * as os from 'os'

const PORT = 3031
const PROFILE_DIR = '/home/z/my-project/.remote-browser-profile'
const WEBLENS_EXT = '/home/z/my-project/Eteryya/07_ТЕХНОЛОГИИ/weblens_extension'
const LOAD_WEBLENS = process.env.WEBLENS_EXT !== '0' // WEBLENS_EXT=0 — без расширения (тяжёлые харвесты)
const GEMINI_DIR = '/home/z/my-project/download/gemini'
const RCLONE = '/home/z/.local/bin/rclone'
const RCLONE_CONF = path.join(os.homedir(), '.config', 'rclone', 'rclone.conf')
const START_URL = 'https://www.google.com/'
const VW = 1280
const VH = 800
const UA =
  'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.0.0 Safari/537.36'

const httpServer = createServer((req, res) => {
  res.writeHead(200, { 'content-type': 'text/plain; charset=utf-8' })
  res.end('browser-service ok')
})

const io = new Server(httpServer, {
  // путь НЕ менять: через него Caddy различает сервисы
  path: '/',
  cors: { origin: '*', methods: ['GET', 'POST'] },
  pingTimeout: 60000,
  pingInterval: 25000,
  maxHttpBufferSize: 64e6,
})

let ctx: any = null
let page: any = null
let quality = 58
let lastFrameAt = 0
let shooting = false
let pendingShot = false
let rcloneProc: ChildProcess | null = null
let rcloneBuf = ''
let authNavDone = false
let authTimer: any = null
let pendingAuthUrl: string | null = null

function log(...a: any[]) {
  console.log(new Date().toISOString().slice(11, 19), ...a)
}

async function safeTitle(): Promise<string> {
  try {
    return await page.title()
  } catch {
    return ''
  }
}

async function pushFrame() {
  if (!page) return
  if (shooting) {
    pendingShot = true
    return
  }
  shooting = true
  try {
    const now = Date.now()
    if (now - lastFrameAt < 110) {
      pendingShot = true
      return
    }
    lastFrameAt = now
    const buf = await page.screenshot({ type: 'jpeg', quality, animations: 'disabled' })
    io.emit('frame', {
      d: buf.toString('base64'),
      t: now,
      u: page.url(),
      ti: await safeTitle(),
    })
  } catch {
    /* страница могла закрыться — пропускаем кадр */
  } finally {
    shooting = false
    if (pendingShot) {
      pendingShot = false
      setTimeout(pushFrame, 120)
    }
  }
}

// мягкий пульс: пока есть зрители — картинка живая
setInterval(() => {
  if (io.engine && (io.engine as any).clientsCount > 0) pushFrame()
}, 1500)

function attach(p: any) {
  page = p
  p.on('framenavigated', async (f: any) => {
    if (f === p.mainFrame()) {
      io.emit('nav', { u: p.url() })
      pushFrame()
    }
  })
  p.on('crash', async () => {
    log('renderer crash → пересоздаю страницу')
    try {
      const np = await ctx.newPage()
      attach(np)
      await np.goto('about:blank').catch(() => {})
    } catch {
      /* ignore */
    }
  })
  p.on('close', () => {
    if (page === p) {
      page = null
      const rest = ctx ? ctx.pages() : []
      if (rest && rest.length) attach(rest[rest.length - 1])
    }
  })
}

async function launch() {
  mkdirSync(path.dirname(PROFILE_DIR), { recursive: true })
  for (let attempt = 1; attempt <= 3; attempt++) {
    try {
      ctx = await chromium.launchPersistentContext(PROFILE_DIR, {
        headless: false, // под Xvfb: настоящий headed-режим — Google не считает его автоматизацией
        viewport: { width: VW, height: VH },
        locale: 'ru-RU',
        timezoneId: 'Europe/Kyiv',
        userAgent: UA,
        args: [
          '--no-sandbox',
          '--disable-dev-shm-usage',
          '--disable-blink-features=AutomationControlled',
          '--disable-infobars',
          '--window-size=1280,800',
          '--lang=ru-RU',
          '--disable-gpu',
          '--js-flags=--max-old-space-size=1536',
          // WebLens прямо в удалённом браузере: Alt+P → MCP 127.0.0.1:8765 → движок
          ...(LOAD_WEBLENS
            ? ['--disable-extensions-except=' + WEBLENS_EXT, '--load-extension=' + WEBLENS_EXT]
            : []),
        ],
      })
      break
    } catch (e) {
      log('launch попытка', attempt, 'упала:', String(e).slice(0, 140))
      // хвостовой Chromium может держать блокировку профиля — снести и подождать
      try {
        spawnSync('pkill', ['-9', '-f', PROFILE_DIR])
      } catch {
        /* ignore */
      }
      if (attempt === 3) throw e
      await new Promise((r) => setTimeout(r, 2500))
    }
  }
  // против детекта автоматизации — на каждый новый документ
  await ctx.addInitScript(() => {
    try {
      Object.defineProperty(navigator, 'webdriver', { get: () => undefined })
      Object.defineProperty(navigator, 'languages', { get: () => ['ru-RU', 'ru', 'en'] })
      ;(window as any).chrome = (window as any).chrome || { runtime: {} }
    } catch {
      /* ignore */
    }
  })
  const pages = ctx.pages()
  const p = pages && pages.length ? pages[pages.length - 1] : await ctx.newPage()
  attach(p)
  try {
    if (!p.url() || p.url() === 'about:blank') {
      await p.goto(START_URL, { waitUntil: 'domcontentloaded', timeout: 45000 })
    }
  } catch {
    /* интернет мог моргнуть — кадр всё равно покажет состояние */
  }
  ctx.on('page', (np: any) => attach(np))
  log('браузер готов:', p.url())
  io.emit('ready', { u: p.url() })
  // если привязка Диска уже стартовала, пока браузер перезапускался — дожать навигацию
  if (pendingAuthUrl && rcloneProc) {
    authNavDone = true
    io.emit('drive', { m: '🗝️ Страница входа открыта — войди в Google и нажми «Разрешить».' })
    p.goto(pendingAuthUrl, { waitUntil: 'domcontentloaded', timeout: 60000 }).catch(() => {})
  }
  pushFrame()
}

/* ---------- Привязка Google Диска (rclone authorize прямо здесь) ---------- */

function saveGdriveToken(json: string) {
  let tok: any
  try {
    tok = JSON.parse(json)
  } catch (e: any) {
    io.emit('drive', { m: '⚠️ Токен пришёл битый, попробуй кнопку ещё раз.' })
    return
  }
  try {
    const confDir = path.dirname(RCLONE_CONF)
    mkdirSync(confDir, { recursive: true })
    let conf = existsSync(RCLONE_CONF) ? readFileSync(RCLONE_CONF, 'utf8') : ''
    if (conf.includes('[gdrive]')) {
      conf = conf.replace(/\[gdrive\]\n[\s\S]*?(?=\n\[|\s*$)/, '')
    }
    if (conf && !conf.endsWith('\n')) conf += '\n'
    conf += `[gdrive]\ntype = drive\nscope = drive\ntoken = ${json}\n`
    writeFileSync(RCLONE_CONF, conf, 'utf8')
    writeFileSync('/home/z/my-project/scripts/rclone_token.json', JSON.stringify(tok, null, 2))
    log('токен gdrive сохранён')
    io.emit('drive', { m: '✅ Google Диск привязан, remote «gdrive» записан. Скажи в чате «готово».' })
    // контрольная проверка живости
    const chk = spawn(RCLONE, ['about', 'gdrive:'], { env: { ...process.env, HOME: os.homedir() } })
    let out = ''
    chk.stdout.on('data', (d: Buffer) => (out += d.toString()))
    chk.on('close', (code) => {
      if (code === 0) io.emit('drive', { m: '✅ Проверка прошла — Диск отвечает. Можно качать.' })
      else io.emit('drive', { m: '⚠️ Токен записан, но проверка не прошла (' + out.trim().slice(0, 120) + ')' })
    })
  } catch (e: any) {
    io.emit('drive', { m: '⚠️ Не удалось записать конфиг: ' + String(e && e.message ? e.message : e) })
  }
}

function startDriveAuth() {
  if (rcloneProc) {
    io.emit('drive', { m: 'Процедура уже идёт — заверши вход в браузере выше.' })
    return
  }
  rcloneBuf = ''
  authNavDone = false
  pendingAuthUrl = null
  io.emit('drive', { m: '🔗 Запускаю привязку… сейчас откроется страница входа Google.' })
  rcloneProc = spawn(RCLONE, ['authorize', 'drive'], {
    cwd: '/home/z/my-project/scripts',
    env: { ...process.env, HOME: os.homedir() },
  })
  authTimer = setTimeout(() => {
    if (rcloneProc) {
      try {
        rcloneProc.kill('SIGKILL')
      } catch {
        /* ignore */
      }
      rcloneProc = null
      io.emit('drive', { m: '⌛ 10 минут истекли. Нажми «Привязать Диск» ещё раз.' })
    }
  }, 10 * 60 * 1000)

  const onData = (chunk: Buffer) => {
    const txt = chunk.toString('utf8')
    log('rclone>', txt.replace(/\s+/g, ' ').trim().slice(0, 300))
    rcloneBuf += txt
    if (rcloneBuf.length > 300000) rcloneBuf = rcloneBuf.slice(-150000)
    if (!authNavDone) {
      const m = rcloneBuf.match(/please go to the following link:\s*(\S+)/i)
      if (m && m[1]) {
        pendingAuthUrl = m[1]
        if (page) {
          authNavDone = true
          io.emit('drive', { m: '🗝️ Войди в Google и нажми «Разрешить». Дальше — само.' })
          page.goto(m[1], { waitUntil: 'domcontentloaded', timeout: 60000 }).catch(() => {})
          pushFrame()
        }
        // если page нет (браузер перезапускается) — launch() сам откроет ссылку
      }
    }
    const t1 = rcloneBuf.indexOf('{"access_token"')
    const t2 = t1 >= 0 ? rcloneBuf.indexOf('<---', t1) : -1
    if (t1 >= 0 && t2 > t1) {
      const json = rcloneBuf.slice(t1, t2).trim()
      if (authTimer) clearTimeout(authTimer)
      try {
        rcloneProc && rcloneProc.kill('SIGKILL')
      } catch {
        /* ignore */
      }
      rcloneProc = null
      saveGdriveToken(json)
    }
  }
  rcloneProc.stdout!.on('data', onData)
  rcloneProc.stderr!.on('data', onData)
  rcloneProc.on('exit', () => {
    rcloneProc = null
  })
}

/* ---------- Автосогласие OAuth: rclone-consent нажимается сам ---------- */

async function tryAutoApprove() {
  if (!rcloneProc || !page) return
  try {
    const u = page.url()
    if (!u.includes('accounts.google.com')) return
    const body = await page.evaluate(() => (document.body && document.body.innerText || '').slice(0, 4000))
    if (!/rclone/i.test(body)) return // жмём только экран согласия rclone, не чужие кнопки
    for (const sel of [
      'button:has-text("Разрешить")',
      '#approve_access',
      'button:has-text("Allow")',
      'button:has-text("Разрешить")',
    ]) {
      const el = await page.$(sel).catch(() => null)
      if (el) {
        const vis = await el.isVisible().catch(() => false)
        if (vis) {
          await el.click()
          log('auto-approve:', sel)
          io.emit('drive', { m: '✅ Нажал «Разрешить» за тебя — токен пишется…' })
          return
        }
      }
    }
  } catch {
    /* страница могла уйти — не страшно */
  }
}
setInterval(() => {
  tryAutoApprove().catch(() => {})
}, 2500)

/* ---------- Сборщик переписок Gemini (через живую сессию) ---------- */

async function harvestGemini(maxChats = 200) {
  io.emit('gemini', { m: '🚀 Открываю Gemini…' })
  try {
    await page.goto('https://gemini.google.com/app', { waitUntil: 'domcontentloaded', timeout: 60000 })
  } catch (e: any) {
    io.emit('gemini', { m: '⚠️ Не открылся Gemini: ' + String(e && e.message ? e.message : e).slice(0, 100), done: true, count: 0 })
    return
  }
  await page.waitForTimeout(5000)
  // Виртуализированный сайдбар: скроллим до полной загрузки списка чатов
  const links = new Map<string, string>()
  for (let round = 0; round < 40; round++) {
    const before = links.size
    let found: { href: string; label: string }[] = []
    try {
      found = await page.$$eval('a[href*="/app/"]', (as: any[]) =>
        as.map((a) => ({ href: a.href, label: (a.innerText || '').trim().split('\n')[0] }))
      )
    } catch {
      /* DOM мог перерисоваться */
    }
    for (const f of found) if (f.href && !links.has(f.href)) links.set(f.href, f.label || '')
    await page
      .evaluate(() => {
        const els = document.querySelectorAll('div,ul,nav,section,aside')
        let scrolled = false
        for (const el of els) {
          const st = window.getComputedStyle(el)
          if (/(auto|scroll)/.test(st.overflowY) && el.scrollHeight > el.clientHeight + 100 && el.clientHeight > 200) {
            el.scrollTop = el.scrollHeight
            scrolled = true
          }
        }
        if (!scrolled) window.scrollTo(0, document.body.scrollHeight)
      })
      .catch(() => {})
    await page.waitForTimeout(1300)
    if (links.size === before && round > 5) break // список стабилен
  }
  const chats = [...links.entries()].filter(([href]) => /\/app\/[a-f0-9]{8,}/i.test(href)).slice(0, maxChats)
  io.emit('gemini', { m: `📋 Найдено чатов: ${chats.length}` })
  mkdirSync(GEMINI_DIR, { recursive: true })
  let n = 0
  for (const [href, label] of chats) {
    n++
    try {
      await page.goto(href, { waitUntil: 'domcontentloaded', timeout: 45000 })
      await page.waitForTimeout(2600)
      const text = await page
        .evaluate(() => {
          const main = document.querySelector('main') || document.body
          return ((main as HTMLElement).innerText || '').trim()
        })
        .catch(() => '')
      const slug = (label || 'chat').replace(/[^\p{L}\p{N}]+/gu, '_').replace(/^_+|_+$/g, '').slice(0, 60) || 'chat'
      const fname = `${String(n).padStart(3, '0')}_${slug}.md`
      const header = `---\nurl: ${href}\nзаголовок: ${label}·дата_сбора: ${new Date().toISOString()}\n---\n\n`
      writeFileSync(path.join(GEMINI_DIR, fname), header + text + '\n', 'utf8')
      io.emit('gemini', { m: `✅ ${n}/${chats.length}: ${label || '(без названия)'}` })
    } catch (e: any) {
      io.emit('gemini', { m: `⚠️ ${n}/${chats.length} не удалось: ${String(e && e.message ? e.message : e).slice(0, 80)}` })
    }
  }
  io.emit('gemini', { m: `🏁 Готово: ${n} чатов → ${GEMINI_DIR}`, done: true, count: n })
  log('gemini harvest:', n, 'чатов')
}

async function forget() {
  try {
    if (ctx) await ctx.close()
  } catch {
    /* ignore */
  }
  ctx = null
  page = null
  try {
    rmSync(PROFILE_DIR, { recursive: true, force: true })
  } catch {
    /* ignore */
  }
  io.emit('drive', { m: '🧹 Куки и профиль стёрты. Браузер перезапускается чистым.' })
  await launch()
}

/* ---------- События от клиента ---------- */

io.on('connection', (s: any) => {
  log('клиент подключился')
  ;(async () => {
    s.emit('hello', { ready: !!page, u: page ? page.url() : '', ti: page ? await safeTitle() : '' })
  })()
  pushFrame()

  s.on('navigate', async (data: any) => {
    try {
      let u = String((data && data.url) || '').trim()
      if (!/^https?:\/\//i.test(u)) u = 'https://' + u
      if (!/^https?:\/\//i.test(u)) return
      await page.goto(u, { waitUntil: 'domcontentloaded', timeout: 45000 })
    } catch (e: any) {
      s.emit('nav_err', { m: String(e && e.message ? e.message : e).slice(0, 200) })
    }
    pushFrame()
  })
  s.on('click', (d: any) => {
    try {
      page.mouse.click(Math.round(+d.x), Math.round(+d.y), { button: d.b === 'right' ? 'right' : 'left' }).catch(() => {})
    } catch {
      /* ignore */
    }
    setTimeout(pushFrame, 160)
  })

  s.on('dblclick', (d: any) => {
    try {
      page.mouse.dblclick(Math.round(+d.x), Math.round(+d.y)).catch(() => {})
    } catch {
      /* ignore */
    }
    setTimeout(pushFrame, 160)
  })

  s.on('wheel', (d: any) => {
    try {
      page.mouse.wheel(Math.round(+d.dx || 0), Math.round(+d.dy || 0)).catch(() => {})
    } catch {
      /* ignore */
    }
    setTimeout(pushFrame, 130)
  })

  s.on('key', (d: any) => {
    try {
      page.keyboard.press(String(d.k)).catch(() => {})
    } catch {
      /* ignore */
    }
    setTimeout(pushFrame, 130)
  })

  s.on('text', (d: any) => {
    try {
      page.keyboard.insertText(String(d.t || '')).catch(() => {})
    } catch {
      /* ignore */
    }
    setTimeout(pushFrame, 110)
  })

  s.on('back', async () => {
    try {
      await page.goBack({ timeout: 15000 })
    } catch {
      /* ignore */
    }
    pushFrame()
  })

  s.on('forward', async () => {
    try {
      await page.goForward({ timeout: 15000 })
    } catch {
      /* ignore */
    }
    pushFrame()
  })

  s.on('reload', async () => {
    try {
      await page.reload({ timeout: 45000 })
    } catch {
      /* ignore */
    }
    pushFrame()
  })

  s.on('quality', (d: any) => {
    const n = Math.round(+d.q)
    if (n >= 30 && n <= 90) quality = n
  })

  s.on('grabtext', async () => {
    try {
      const t = await page.evaluate(
        () => ((document.body && document.body.innerText) || '').slice(0, 3000)
      )
      s.emit('grabbed_text', { u: page ? page.url() : '', t })
    } catch (e: any) {
      s.emit('grabbed_text', { u: page ? page.url() : '', t: 'ERR ' + String(e && e.message) })
    }
  })

  // мягкая пересборка страницы после краша (по запросу клиента)
  s.on('revive', async () => {
    try {
      const np = await ctx.newPage()
      attach(np)
      await np.goto('about:blank').catch(() => {})
      pushFrame()
    } catch {
      /* ignore */
    }
  })

  // --- автоматизация из локальных скриптов (eval с токеном из окружения) ---
  s.on('eval', async (d: any, ack?: any) => {
    const reply = (payload: any) => {
      try {
        if (typeof ack === 'function') ack(payload)
      } catch {
        /* ignore */
      }
    }
    try {
      if (!page) return reply({ ok: false, e: 'no page' })
      const wantTok = String((d && d.t) || '')
      if (!process.env.AUTOMATE_TOKEN || wantTok !== process.env.AUTOMATE_TOKEN)
        return reply({ ok: false, e: 'auth' })
      const code = String((d && d.code) || 'null')
      if (d && d.frames) {
        // выполнить в первом фрейме, где код вернёт не-null (кросс-доменные iframe)
        let out: any = null
        let src = ''
        for (const f of page.frames()) {
          try {
            const r = await f.evaluate(code)
            if (r !== null && r !== undefined) {
              out = r
              src = f.url()
              break
            }
          } catch {
            /* этот фрейм не подходит */
          }
        }
        return reply({ ok: true, frame: src, r: out === undefined ? null : out })
      }
      const r = await page.evaluate(code)
      reply({ ok: true, r: r === undefined ? null : r })
    } catch (e: any) {
      reply({ ok: false, e: String(e && e.message ? e.message : e).slice(0, 600) })
    }
    setTimeout(pushFrame, 160)
  })

  s.on('drive_auth', () => startDriveAuth())
  s.on('gemini_harvest', () => {
    harvestGemini().catch((e) =>
      io.emit('gemini', { m: '⚠️ Сборщик упал: ' + String(e && e.message ? e.message : e).slice(0, 120) })
    )
  })
  s.on('forget', () => {
    forget().catch(() => {})
  })
  s.on('disconnect', () => log('клиент ушёл'))
})

launch().catch((e) => {
  console.error('LAUNCH FAIL:', e)
})
httpServer.listen(PORT, () => log('browser-service слушает', PORT))

process.on('uncaughtException', (e) => console.error('uncaught:', e))
process.on('unhandledRejection', (e) => console.error('unhandled:', e))
