def calculate_confidence(theme, keywords, clusters):
# Если есть семантическая тема от LLM — доверие выше
base = 0.85 if theme.get("semantic") else 0.6
# Бонус за количество ключевых слов
if keywords and len(keywords) > 5:
base += 0.05
if clusters and len(clusters) > 2:
base += 0.05
return min(base, 1.0)

# ... после того, как получены text, meta, theme, keywords, clusters

output = {
"status": "success",
"confidence": calculate_confidence(theme, keywords, clusters),
"data": {
"text": text,
"meta": meta,
"theme": theme,
"keywords": keywords,
"clusters": clusters
}
}

print(json.dumps(output, ensure_ascii=False))
✅ 4. Валидатор на основе JSON Schema
python
