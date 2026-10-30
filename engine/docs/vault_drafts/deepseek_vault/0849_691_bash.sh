curl https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions \
-H "Authorization: Bearer YOUR_API_KEY" \
-H "Content-Type: application/json" \
-d '{
"model": "qwen3-max",
"messages": [{"role": "user", "content": "Привет!"}]
}'
