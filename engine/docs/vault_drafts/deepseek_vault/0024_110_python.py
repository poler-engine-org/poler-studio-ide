import language_tool_python

# Выберите язык: 'uk-UA' для украинского, 'ru-RU' для русского
LANG = 'uk-UA'  # поменяйте на 'ru-RU', если нужно

# Инициализируем инструмент (он скачает нужные правила при первом запуске)
tool = language_tool_python.LanguageTool(LANG)

# Пример текста для проверки
text = "Привіт, я перевіряю правопис згідно нових правил."

# Проверяем
matches = tool.check(text)

if not matches:
    print("✅ Ошибок не найдено!")
else:
    print(f"🔍 Найдено {len(matches)} ошибок:\n")
for i, match in enumerate(matches, 1):
print(f"{i}. {match.message}")
print(f"   Фрагмент: ...{match.context}...")
if match.replacements:
print(f"   ➡️  Предложения: {', '.join(match.replacements[:3])}")
print("-" * 50)
