#!/usr/bin/env bash
# Смоук-тест MCP-сервера poler-api (stdio, JSON-RPC 2.0).
# Проверяет то же, что увидит Claude Desktop / Cursor / любой MCP-клиент:
# initialize -> tools/list -> подсчёт инструментов.
# Запуск: bash generic_stdio_smoke.sh
export PATH="$HOME/.local/bin:$PATH"
command -v poler-api >/dev/null 2>&1 || { echo "ОШИБКА: poler-api не в PATH"; exit 1; }
command -v poler-engine >/dev/null 2>&1 || { echo "ОШИБКА: poler-engine не в PATH (прокси порождает движок как дочерний процесс)"; exit 1; }

{
  printf '%s\n' \
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke","version":"1.0"}}}' \
    '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
    '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
  sleep 5
} | poler-api mcp 2>/dev/null | python3 -c '
import json, sys
init_ok, tools = False, []
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        obj = json.loads(line)
    except Exception:
        continue
    if obj.get("id") == 1 and "result" in obj:
        init_ok = True
    if obj.get("id") == 2 and isinstance(obj.get("result"), dict):
        tools = obj["result"].get("tools", [])
names = [t.get("name", "?") for t in tools]
NATIVE = {"poler_matrix_det", "poler_matrix_eigen_sturm"}
native = [n for n in names if n in NATIVE]
engine = [n for n in names if n not in native]
print("initialize :", "OK" if init_ok else "FAIL")
print("инструменты:", len(names), "(движок", len(engine), "+ нативные шлюза", len(native), ")")
print("нативные   :", ", ".join(native) if native else "НЕ НАЙДЕНЫ")
print("примеры    :", ", ".join(names[:10]))
'

# Пример прямого вызова нативного Штурм-ядра (то, что модель сделает сама):
# {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"poler_matrix_eigen_sturm",
#  "arguments":{"source":"tridiag:2,-1,256"}}}
