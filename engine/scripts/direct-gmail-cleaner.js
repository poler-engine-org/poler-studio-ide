const { spawn } = require('child_process');
const http = require('http');
const M = require('/home/vitalij/Стільниця/poler-engine/dev-stand/gcp-cdp-machinery.js');

const CHROME = '/home/vitalij/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome';
const PROFILE = '/home/vitalij/.cache/poler-engine/google-profile';
const CDP = 9226;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

(async () => {
  console.log('[1/4] Запуск внутреннего Chromium с твоим Google-профилем...');
  const child = spawn(CHROME, [
    `--user-data-dir=${PROFILE}`,
    `--remote-debugging-port=${CDP}`,
    '--remote-debugging-address=127.0.0.1',
    '--no-first-run',
    '--no-default-browser-check',
    '--no-sandbox',
    '--headless=new',
    '--disable-gpu',
    'about:blank'
  ], { stdio: 'ignore' });

  for (let i = 0; i < 30; i++) {
    await sleep(400);
    try {
      await M.getJsonPort(CDP, '/json/version');
      break;
    } catch (_) {}
  }

  console.log('[2/4] Подключение к странице и навигация в Gmail...');
  let cdp = await M.connectPageRetry(CDP, 5);
  await cdp.call('Page.navigate', { url: 'https://mail.google.com/mail/u/0/#search/from%3Acreativefabrica.com' });
  await sleep(10000);

  console.log('[3/4] Проверка статуса Gmail DOM...');
  const text = await M.evalPage(cdp, 'document.title + " | " + (document.body ? document.body.innerText.slice(0, 300).replace(/\\s+/g, " ") : "")');
  console.log('Текущий экран:', text);

  child.kill('SIGKILL');
  console.log('[4/4] Завершено.');
})().catch(e => {
  console.error('Ошибка:', e.message);
  process.exit(1);
});
