# POLER Remote Browser — человеко-удобный GUI поверх poler-engine

Интерактивный браузер, живущий рядом с движком, которым пользователь
управляет мышью из веб-превью: клики, колесо, клавиатура, Ctrl+V.
Один вход в Google внутри него даёт сразу всё:

- **токен Google Диска** — кнопка «Привязать Диск» запускает
  `rclone authorize drive` рядом с движком, страница согласия открывается
  в этом же браузере (localhost-редирект OAuth попадает куда надо),
  согласие нажимается автоматически, токен пишется в `rclone.conf`;
- **сессионные куки** — Gemini/Colab/Takeout доступны через живую сессию;
- **WebLens внутри** — расширение грузится флагом `--load-extension`,
  Alt+P в этом браузере стримит контент в MCP движка (`127.0.0.1:8765`).

## Состав

```
gui/remote-browser/
├── browser-service/     # mini-service: Playwright + socket.io, порт 3031
│   ├── index.ts         # стриминг JPEG-кадров, ввод, rclone-привязка,
│   │                    # автосогласие OAuth, сборщик переписок Gemini
│   └── package.json     # bun run dev (bun --hot)
└── app/
    └── page.tsx         # Next.js 16 страница-«кокон» (shadcn/ui:
                         # Button/Input/Select + socket.io-client)
```

## Запуск

```bash
# 1) виртуальный дисплей — headed-режим обязателен:
#    headless Google блокирует ("небезопасный браузер"), headed — принимает
Xvfb :99 -screen 0 1280x900x24 -nolisten tcp &

# 2) сервис (PLAYWRIGHT-профиль персистентен — куки переживают рестарты)
cd gui/remote-browser/browser-service
bun install
DISPLAY=:99 bun run dev          # слушает 3031

# 3) движок как MCP-сервер (инструменты полer_chunk и др.)
poler-engine --mcp-http 8765 --mcp-token <твой-токен> &

# 4) страница: socket.io-клиент ходит через шлюз превью:
#    io('/?XTransformPort=3031')  — см. app/page.tsx
```

`WEBLENS_EXT=0 bun run dev` — стартовать БЕЗ расширения (тяжёлые харвесты
на гигантских DOM-страницах стабильнее без content-скриптов).

## Связь с движком

| Контур | Канал |
|---|---|
| WebLens (Alt+P) | `POST http://127.0.0.1:8765/mcp` · JSON-RPC `tools/call poler_chunk {source, title, url, content}` · `Authorization: Bearer …` |
| Диск | `rclone` remote `gdrive` (токен из кнопки «Привязать Диск»), выгрузка порциями → `poler-engine --poler-patch ARCHIVE.poler --manifest {add:[{name,file}]}` |
| Сбор Gemini | `gemini_harvest` по socket.io: авто-скролл виртуализированного сайдбара → каждый чат → Markdown с якорем url |

## Памятка

- Память рендера ограничена (`--js-flags=--max-old-space-size=1536`);
  краш страницы самолечится пересозданием вкладки (`page.on('crash')`).
- «Забыть сессию» стирает профиль целиком — экстренный выход.
- Кадры — JPEG quality 40–75 на выбор; задержка ~50–150 мс.

## Автоматизация (socket-событие `eval`)

Помимо ручного управления мышью/клавиатурой сервис принимает
`eval { t: AUTOMATE_TOKEN, code, frames }` — выполнение JS в активной
странице удалённого браузера с возвратом результата (ack). Режим
`frames: true` обходит все фреймы (кросс-доменные output-iframe Colab,
OAuth-диалоги) и возвращает первый не-null ответ. Токен задаётся в
`.env` (см. `.env.example`) — без него `eval` отклоняется. Примеры —
`scripts/remote_eval.mjs`, `remote_key.mjs`, `remote_click.mjs`,
`remote_shot.mjs` в корне проекта-превью.

Так настраиваются Colab-блокноты и Takeout-экспорты без ручной прокрутки:
страницы открываются в живой сессии пользователя, кнопки нажимаются
скриптом, кадр можно снять и проанализировать (VLM).

### Траблшутинг: Turbopack падает с OOM на маленькой машине (4 ГБ RAM)

Симптом: `Failed to write app endpoint /page` → `PostCssTransformedAsset::process`
→ `unexpected end of file`, а в dmesg — oom-kill. Tailwind v4 по умолчанию
сканирует источники классов по ВСЕМУ корню проекта — если рядом лежат большие
деревья (репозитории, зеркала Диска), oxide-сканер съедает гигабайты.

Лечение (в `src/app/globals.css`):

```css
@import "tailwindcss" source(none);
@source "../**/*.{ts,tsx}";   /* только код приложения */
```

и продакшн-запуск вместо dev: `bun run build && bun .next/standalone/server.js`.
