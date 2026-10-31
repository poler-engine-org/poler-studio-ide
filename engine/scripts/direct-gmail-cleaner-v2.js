const { spawn } = require('child_process');
const M = require('/home/vitalij/Стільниця/poler-engine/dev-stand/gcp-cdp-machinery.js');

const CHROME = '/home/vitalij/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome';
const PROFILE = '/home/vitalij/.cache/poler-engine/google-profile';
const sleep = M.sleep;

(async () => {
  console.log('[1/4] Поднимаем Chromium через M.launchChromium()...');
  const { child, cdpPort } = await M.launchChromium();
  console.log('CDP порт:', cdpPort);

  try {
    let cdp = await M.connectPageRetry(cdpPort, 5);
    console.log('[2/4] Открываем Gmail (from:creativefabrica.com)...');
    await cdp.call('Page.navigate', { url: 'https://mail.google.com/mail/u/0/#search/from%3Acreativefabrica.com' });
    await sleep(12000);

    // reconnect after navigation
    cdp = await M.connectPageRetry(cdpPort, 5);

    console.log('[3/4] Читаем страницу...');
    const dom = await M.evalPage(cdp, 'document.title + " || " + (document.body ? document.body.innerText.slice(0, 300).replace(/\\s+/g, " ") : "")');
    console.log('ЭКРАН GMAIL:\n', dom);

  } finally {
    try { child.kill('SIGKILL'); } catch (_) {}
  }
})().catch(e => {
  console.error('Ошибка:', e.message);
  process.exit(1);
});
