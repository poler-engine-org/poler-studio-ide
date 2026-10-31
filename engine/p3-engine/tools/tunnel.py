#!/usr/bin/env python3
"""
Python Public Tunnel Runner
Bypasses firewall/ISP port blocking by using standard HTTPS (Port 443).
Supported backends: Cloudflare Quick Tunnels, Pinggy (SSH/443).
"""

import sys
import subprocess
import shutil
import re
import time
import argparse

def run_cloudflare_tunnel(port):
    print(f"[*] Запуск туннеля через Cloudflare для localhost:{port} (через стандартный порт 443 HTTPS)...")
    cmd = ["cloudflared", "tunnel", "--url", f"http://localhost:{port}"]
    proc = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1
    )
    url = None
    start_time = time.time()
    try:
        while time.time() - start_time < 20:
            line = proc.stdout.readline()
            if not line:
                if proc.poll() is not None:
                    break
                time.sleep(0.1)
                continue
            
            # Match trycloudflare url (ignore api.trycloudflare.com)
            match = re.search(r"https://([a-zA-Z0-9-]+\.trycloudflare\.com)", line)
            if match and "api.trycloudflare.com" not in match.group(1):
                url = "https://" + match.group(1)
                break
        
        if url:
            print("\n" + "=" * 60)
            print(f"🚀 ПУБЛИЧНЫЙ URL ГОТОВ:")
            print(f"👉 {url}")
            print(f"👉 Перенаправление на -> http://localhost:{port}")
            print("=" * 60 + "\n")
            print("Нажмите Ctrl+C для остановки туннеля.\n")
            while True:
                line = proc.stdout.readline()
                if not line and proc.poll() is not None:
                    break
        else:
            print("[-] Не удалось получить ссылку Cloudflare. Переключаемся на резервный SSH туннель...")
            proc.terminate()
            run_ssh_tunnel(port)
    except KeyboardInterrupt:
        print("\n[*] Туннель остановлен.")
        proc.terminate()

def run_ssh_tunnel(port):
    print(f"[*] Запуск SSH туннеля через порт 443 для localhost:{port}...")
    cmd = [
        "ssh",
        "-o", "StrictHostKeyChecking=no",
        "-o", "ServerAliveInterval=30",
        "-p", "443",
        f"-R0:localhost:{port}",
        "-T",
        "a.pinggy.io"
    ]
    try:
        proc = subprocess.Popen(cmd)
        proc.wait()
    except KeyboardInterrupt:
        print("\n[*] Туннель остановлен.")
        proc.terminate()

def main():
    parser = argparse.ArgumentParser(description="Публичный туннель без блокировок провайдера")
    parser.add_argument("port", type=int, nargs="?", default=17839, help="Локальный порт (по умолчанию: 17839)")
    parser.add_argument("--mode", choices=["cloudflare", "ssh", "auto"], default="auto", help="Провайдер туннеля")
    args = parser.parse_args()

    port = args.port
    
    if args.mode == "ssh" or (args.mode == "auto" and not shutil.which("cloudflared")):
        run_ssh_tunnel(port)
    else:
        run_cloudflare_tunnel(port)

if __name__ == "__main__":
    main()
