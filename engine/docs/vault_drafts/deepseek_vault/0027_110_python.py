import requests
import xml.etree.ElementTree as ET

# Ссылка на файл с правилами для русского языка
url = "https://raw.githubusercontent.com/languagetool-org/languagetool/master/languagetool-language-modules/ru/src/main/resources/org/languagetool/rules/ru/grammar.xml"

# Скачиваем файл
response = requests.get(url)
response.encoding = 'utf-8'
xml_content = response.text

# Парсим XML
root = ET.fromstring(xml_content)

# Извлекаем все правила
rules = []
for rule in root.findall('.//rule'):
rule_id = rule.get('id')
description = rule.find('description')
message = rule.find('message')
pattern = rule.find('.//pattern')

rules.append({
'id': rule_id,
'description': description.text if description is not None else None,
'message': message.text if message is not None else None,
        'pattern': pattern is not None  # сам паттерн требует дополнительного разбора
})

# Выводим первые 5 правил
for r in rules[:5]:
print(f"ID: {r['id']}")
print(f"Описание: {r['description']}")
    print(f"Сообщение: {r['message']}")
print("-" * 40)
