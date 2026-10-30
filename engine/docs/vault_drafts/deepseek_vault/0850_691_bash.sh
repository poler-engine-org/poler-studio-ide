export DEEPSEEK_API_KEY="sk-ваш_ключ"
curl https://api.deepseek.com/chat/completions \
-H "Content-Type: application/json" \
-H "Authorization: Bearer $DEEPSEEK_API_KEY" \
-d '{
"model": "deepseek-chat",
        "messages": [{"role": "user", "content": "Привет, как дела?"}]
}'
