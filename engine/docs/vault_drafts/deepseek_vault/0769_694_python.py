import numpy as np
import json
from collections import deque

class NullTransformerPrototype:
def __init__(self):
# 1. Базовые архетипы (20 для начала)
self.archetypes = [
"order", "chaos", "connection", "separation",
"growth", "decay", "protection", "attack",
"knowledge", "mystery", "creation", "destruction",
"truth", "deception", "harmony", "conflict",
"freedom", "control", "life", "death"
]

# 2. Матрица весов (изначально случайная)
self.W = np.random.randn(len(self.archetypes),
len(self.archetypes)) * 0.1

# 3. Память
self.memory = deque(maxlen=1000)

# 4. Словарь простого маппинга
self.word_map = self.create_word_map()

def create_word_map(self):
# Простой маппинг слов на архетипы
return {
"help": ["connection", "protection"],
"harm": ["attack", "destruction"],
"learn": ["knowledge", "growth"],
"hide": ["mystery", "protection"],
"create": ["creation", "order"],
"destroy": ["destruction", "chaos"],
"love": ["connection", "harmony"],
"hate": ["separation", "conflict"],
"free": ["freedom", "chaos"],
"control": ["control", "order"]
}

def text_to_vector(self, text):
vector = np.zeros(len(self.archetypes))
words = text.lower().split()

for word in words:
if word in self.word_map:
for archetype in self.word_map[word]:
idx = self.archetypes.index(archetype)
vector[idx] += 1.0

# Нормализация
norm = np.linalg.norm(vector)
return vector / norm if norm > 0 else vector

def process(self, input_text):
# Входной вектор
V_in = self.text_to_vector(input_text)

# Применяем матрицу
V_out = np.dot(self.W, V_in)

# Мягкая активация
V_out = np.tanh(V_out)

# Поиск в памяти
resonance = self.find_resonance(V_in)
if resonance:
V_out = 0.7 * V_out + 0.3 * resonance

# Сохраняем в память
self.memory.append((V_in.copy(), V_out.copy()))

# Генерация ответа
return self.vector_to_response(V_out)

def find_resonance(self, vector):
if not self.memory:
return None

best_similarity = -1
best_output = None

for mem_in, mem_out in self.memory:
similarity = np.dot(vector, mem_in)
if similarity > best_similarity:
best_similarity = similarity
best_output = mem_out

return best_output if best_similarity > 0.5 else None

def vector_to_response(self, vector):
# Находим топ-3 архетипа
top_idx = np.argsort(vector)[-3:][::-1]

response_parts = []
for idx in top_idx:
if vector[idx] > 0.2:
archetype = self.archetypes[idx]
response_parts.append(archetype)

# Превращаем в предложение (простейший вариант)
if not response_parts:
return "I need more information."

response = f"This relates to {', '.join(response_parts[:-1])} and {response_parts[-1]}."
return response

def learn(self, input_text, feedback_text, feedback_score):
# Обратное распространение фидбека
V_in = self.text_to_vector(input_text)
V_feedback = self.text_to_vector(feedback_text)

# Ошибка
V_current = np.dot(self.W, V_in)
error = V_feedback - V_current

# Обновление весов
self.W += 0.01 * np.outer(error, V_in) * feedback_score

# Ограничение весов
self.W = np.clip(self.W, -1.0, 1.0)

# Использование
ai = NullTransformerPrototype()

# Первое взаимодействие
response = ai.process("help me learn")
print(f"AI: {response}")  # "This relates to knowledge, growth and connection."

# Пользователь даёт фидбек
ai.learn("help me learn", "teach me something new", 1.0)

# Следующий запрос
response = ai.process("I want to create something")
print(f"AI: {response}")
