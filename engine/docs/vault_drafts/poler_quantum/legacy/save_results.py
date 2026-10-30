import json, sys
from datetime import datetime

# Аргументы из GitHub Actions
mode = sys.argv[1]
data = sys.argv[2]

# Сохраняем результаты в общий JSON
timestamp = datetime.utcnow().isoformat()
record = {"time": timestamp, "mode": mode, "results": eval(data)}

try:
    with open("results.json", "r", encoding="utf-8") as f:
        history = json.load(f)
except FileNotFoundError:
    history = []

history.append(record)

with open("results.json", "w", encoding="utf-8") as f:
    json.dump(history, f, ensure_ascii=False, indent=2)
