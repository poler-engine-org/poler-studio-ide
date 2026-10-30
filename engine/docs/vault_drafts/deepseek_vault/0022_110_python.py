import language_tool_python

# Для украинского
tool = language_tool_python.LanguageTool('uk-UA')
# Для русского
# tool = language_tool_python.LanguageTool('ru-RU')

text = "Привіт, я перевіряю правопис згідно нових правил."

matches = tool.check(text)

if not matches:
    print("✅ Ошибок не найдено!")
else:
for match in matches:
print(f"❌ Ошибка: {match.message}")
print(f"   В тексте: ...{match.context}...")
if match.replacements:
print(f"   ➡️  Исправьте на: {', '.join(match.replacements[:3])}")
print("-" * 50)
3. Запустите
bash
