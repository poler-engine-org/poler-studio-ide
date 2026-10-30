curl -X POST http://localhost:8080/analyze \
-H "Content-Type: application/json" \
-d '{"file": "/path/to/file.txt", "keyword": "error", "top": 3}'
Поиск grep в файле
bash
