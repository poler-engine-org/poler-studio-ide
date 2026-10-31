const http = require('http');
const WebSocket = require('ws');
const fs = require('fs');
const path = require('path');

async function getGeminiTab() {
  return new Promise((resolve, reject) => {
    http.get('http://localhost:9222/json', (res) => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => {
        const tabs = JSON.parse(data);
        const geminiTab = tabs.find(t => t.url && t.url.includes('gemini.google.com'));
        resolve(geminiTab);
      });
    }).on('error', reject);
  });
}

async function run() {
  const tab = await getGeminiTab();
  if (!tab) {
    console.error("Gemini tab not found!");
    process.exit(1);
  }
  console.log("Connected to tab:", tab.title, tab.url);

  const ws = new WebSocket(tab.webSocketDebuggerUrl);
  let msgId = 1;
  const pending = new Map();

  function send(method, params = {}) {
    return new Promise((resolve) => {
      const id = msgId++;
      pending.set(id, resolve);
      ws.send(JSON.stringify({ id, method, params }));
    });
  }

  ws.on('message', (msg) => {
    const data = JSON.parse(msg);
    if (data.id && pending.has(data.id)) {
      const resolve = pending.get(data.id);
      pending.delete(data.id);
      resolve(data.result);
    }
  });

  ws.on('open', async () => {
    console.log("CDP WebSocket opened. Evaluating Gemini state...");

    // 1. Check if user is logged in
    const checkLogin = await send('Runtime.evaluate', {
      expression: `
        (() => {
          const bodyText = document.body.innerText;
          const isSignIn = bodyText.includes('Sign in') || bodyText.includes('Увійти') || bodyText.includes('Войти');
          const sidebarLinks = Array.from(document.querySelectorAll('a[href*="/app/"], div[role="button"], div[data-test-id]'))
            .map(el => ({ text: el.innerText.trim(), href: el.href || el.getAttribute('href') }))
            .filter(x => x.text && x.text.length > 2);
          
          return {
            url: window.location.href,
            title: document.title,
            isSignIn: isSignIn,
            sidebarItemsCount: sidebarLinks.length,
            sidebarSample: sidebarLinks.slice(0, 15)
          };
        })()
      `,
      returnByValue: true
    });

    console.log("Gemini Status:", JSON.stringify(checkLogin.result.value, null, 2));
    ws.close();
  });
}

run().catch(console.error);
