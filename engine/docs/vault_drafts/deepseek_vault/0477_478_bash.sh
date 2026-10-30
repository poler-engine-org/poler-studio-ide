# Сервер, краулит 10 случайных статей Википедии и ждёт клиента
python poler-seed-scanner.py --mode server --source wiki --pages 10

# Сервер по конкретной теме
python poler-seed-scanner.py --mode server --source wiki --topic "quantum physics" --pages 5

# Клиент подключается на порт по умолчанию
python poler-seed-scanner.py --mode client

# Только краулинг и сохранение в файл
python poler-seed-scanner.py --mode scan --source wiki --pages 5 --output data.json
