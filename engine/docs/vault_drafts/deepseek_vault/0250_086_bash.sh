# Зупинити старий сервер (знайти PID і вбити)
pkill -f "python3 -m http.server 8080"

# Запустити новий у потрібній папці
cd /шлях/до/нової/папки
python3 -m http.server 8080 --directory "$PWD" &
