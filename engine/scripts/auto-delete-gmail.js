const { spawn } = require('child_process');
const http = require('http');
const net = require('net');
const crypto = require('crypto');
const fs = require('fs');

const CHROME = '/home/vitalij/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome';
const PROFILE = '/home/vitalij/.cache/poler-engine/google-profile';
const CDP = 9226;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function getJson(port, p) {
  return new Promise((res, rej) => {
    http.get({ host: '127.0.0.1', port, path: p, timeout: 3000 }, (r) => {
      let b = '';
      r.on('data', (c) => (b += c));
      r.on('end', () => {
        try { res(JSON.parse(b)); } catch (e) { rej(e); }
      });
    }).on('error', rej);
  });
}

class MiniWs {
  constructor(sock) {
    this.sock = sock;
    this.buf = Buffer.alloc(0);
    this.onMessage = null;
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
      sock.on('data', (d) => {
        if (!handshaked) {
          this.buf = Buffer.concat([this.buf || Buffer.alloc(0), d]);
          const i = this.buf.indexOf('\r\n\r\n');
          if (i === -1) return;
          handshaked = true;
          const ws = new MiniWs(sock);
          resolve(ws);
        } else if (this.onMessage) {
          // simple feed
        }
      });
    });
  }
}

console.log('Подготовка к прямому открытию Gmail через внутренний Chromium...');
