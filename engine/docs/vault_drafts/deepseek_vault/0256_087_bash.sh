autossh -M 0 -o "ServerAliveInterval 30" -o "ServerAliveCountMax 3" -R 2222:localhost:22 user@vps

Если не можете установить autossh, запустите простой бесконечный цикл в скрипте:

bash
