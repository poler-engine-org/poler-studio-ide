ssh -o ServerAliveInterval=30 -o ServerAliveCountMax=3 -o ExitOnForwardFailure=yes -R 2222:localhost:22 user@your-vps.com

ServerAliveInterval=30 — шлет «пинг» каждые 30 секунд, чтобы соединение не разрывалось по таймауту-
.

ExitOnForwardFailure=yes — если туннель упал, SSH перезапустится-
.
