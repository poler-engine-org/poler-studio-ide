# Задача: "Кошка сидит на коврике. Коврик красный. Какого цвета кошка?"

mind = CompleteMindOS()

# Кодирование входных данных
input_data = {
'entities': {
'cat': {'type': 'animal', 'location': 'on_mat'},
'mat': {'type': 'object', 'color': 'red', 'location': 'floor'}
},
'relations': [
('cat', 'on', 'mat'),
('mat', 'color', 'red')
],
'query': "What color is the cat?"
}

# Процесс мышления
reasoning_steps = []

# 1. Перцептуальная обработка
perceptual = mind.processes['perception'].execute(input_data)
reasoning_steps.append(("perception", perceptual))

# 2. Извлечение знания из памяти
memory_result = mind.processes['memory'].retrieve(
query="cat typical_colors",
context=perceptual
)
reasoning_steps.append(("memory", memory_result))

# 3. Логический вывод
reasoning_result = mind.processes['reasoning'].infer([
"IF X is on Y AND Y is red THEN X color is unknown",
"cats are typically black, white, gray, or orange",
"red mat suggests maybe contrast"
])
reasoning_steps.append(("reasoning", reasoning_result))

# 4. Креативное предположение
creative = mind.processes['creativity'].generate_hypotheses([
"cat might be black for contrast",
"cat might be white to stand out",
"cat might be multicolored"
])
reasoning_steps.append(("creativity", creative))

# 5. Мета-когнитивная оценка
metacognitive = mind.processes['metacognition'].evaluate([
"we don't have direct evidence",
"typical colors are not definitive",
"best answer: unknown but likely contrasting"
])
reasoning_steps.append(("metacognition", metacognitive))

# Формирование ответа
response = mind.formulate_response({
'answer': "The cat's color is not specified, but typical cat colors are black, white, gray, or orange.",
'confidence': 0.7,
'reasoning_chain': reasoning_steps
})

print(response)
