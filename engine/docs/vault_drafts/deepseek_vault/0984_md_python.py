"""
Continuous Semantic Encoding (CSE)
Заменяет BPE/токенизацию на непрерывное семантическое кодирование
"""

import numpy as np
from dataclasses import dataclass
from typing import List, Dict, Tuple, Optional
import hashlib
import struct
from scipy.spatial import KDTree

@dataclass
class SemanticAxis:
"""Ось семантического пространства"""
id: int
name: str  # например: "существительное", "действие", "эмоция", "физическая величина"
weight: float  # вес оси в общем пространстве (0.0-1.0)
base_vector: np.ndarray  # базовый вектор оси (64-256 измерений)

@dataclass
class SemanticField:
"""Семантическое поле - область пространства со схожим значением"""
centroid: np.ndarray  # центр поля
radius: float  # радиус поля
context_hash: int  # хэш контекста, в котором это поле актуально
activation_count: int  # счетчик активаций (для адаптации)

class CSEEncoder:
"""
Непрерывный семантический энкодер

Принципы:
1. Текст → семантические координаты в N-мерном пространстве
2. Координаты вычисляются через синусоидные проекции
3. Контекст влияет на трансформацию пространства
4. Память предыдущих кодирований адаптирует пространство
"""

def __init__(self,
space_dimensions: int = 128,
n_axes: int = 32,
adaptive_memory_size: int = 10000):
"""
Args:
space_dimensions: размерность семантического пространства
n_axes: количество семантических осей
adaptive_memory_size: размер адаптивной памяти
"""
self.space_dim = space_dimensions
self.n_axes = n_axes

# Инициализация семантических осей через распределенные синусоидные частоты
self.axes = self._initialize_semantic_axes()

# Адаптивная память: хранит частые паттерны и их семантические координаты
self.memory = AdaptiveSemanticMemory(capacity=adaptive_memory_size)

# Синусоидные базисы для разных частотных диапазонов
self.frequency_basis = self._create_frequency_basis()

# Контекстный трансформер: динамически подстраивает пространство под контекст
self.context_transformer = ContextTransformer(space_dimensions)

# Кэш быстрых проекций
self.projection_cache = {}

def _initialize_semantic_axes(self) -> List[SemanticAxis]:
"""Инициализация осей через равномерное распределение по гиперсфере"""
axes = []
for i in range(self.n_axes):
# Базовый вектор оси - равномерное распределение на гиперсфере
vec = np.random.randn(self.space_dim)
vec = vec / np.linalg.norm(vec)

# Каждая ось получает уникальную частоту синусоиды
freq = 2**((i % 16) / 4.0)  # Экспоненциальное распределение частот

axis = SemanticAxis(
id=i,
name=f"axis_{i}_freq_{freq:.2f}",
weight=1.0 / (i + 1),  # Веса убывают
base_vector=vec * freq  # Масштабируем базовый вектор частотой
)
axes.append(axis)
return axes

def _create_frequency_basis(self) -> np.ndarray:
"""Создание синусоидного базиса для преобразования текста"""
# Многомерный синусоидный базис
basis_size = 64
basis = np.zeros((basis_size, self.space_dim))

for i in range(basis_size):
freq = 1.0 + i * 0.5
for j in range(self.space_dim):
basis[i, j] = np.sin(freq * j + i * 0.1)

# Ортогонализация базиса
q, _ = np.linalg.qr(basis.T)
return q.T

def text_to_semantic_vector(self,
text: str,
context: Optional[str] = None,
temperature: float = 0.7) -> np.ndarray:
"""
Преобразование текста в семантический вектор

Args:
text: входной текст
context: контекст (предыдущий текст, тема и т.д.)
temperature: "творчество" кодирования (0.0-1.0)

Returns:
Семантический вектор размерности space_dim
"""
# 1. Быстрая проверка в памяти
text_hash = self._hash_text(text, context)
if text_hash in self.projection_cache:
cached_vec, timestamp = self.projection_cache[text_hash]
# "старение" кэша - легкая трансформация со временем
age_factor = 0.99  # каждый вызов слегка меняет вектор
return cached_vec * age_factor + np.random.randn(self.space_dim) * 0.01 * temperature

# 2. Создание начального вектора через синусоидные проекции символов
char_vectors = self._project_chars_to_sinusoids(text)

# 3. Свертка последовательности с адаптивными весами
seq_vector = self._adaptive_sequence_convolution(char_vectors)

# 4. Проекция на семантические оси с учетом их весов
semantic_projection = np.zeros(self.space_dim)

for axis in self.axes:
# Синусоидная проекция: sin(угол между векторами * вес оси)
angle = np.dot(seq_vector, axis.base_vector) / (
np.linalg.norm(seq_vector) * np.linalg.norm(axis.base_vector) + 1e-8
)
contribution = np.sin(angle * np.pi) * axis.weight

# Добавляем вклад оси с ее частотной характеристикой
semantic_projection += axis.base_vector * contribution

# 5. Применение контекстной трансформации
if context:
context_vector = self.text_to_semantic_vector(context, None, temperature=0.5)
semantic_projection = self.context_transformer.transform(
semantic_projection, context_vector
)

# 6. "Температурное" добавление шума для вариативности
if temperature > 0:
noise = np.random.randn(self.space_dim) * temperature * 0.1
semantic_projection += noise

# Нормализация на сфере
norm = np.linalg.norm(semantic_projection)
if norm > 0:
semantic_projection = semantic_projection / norm * np.sqrt(self.space_dim)

# 7. Сохранение в кэш и памяти
self.projection_cache[text_hash] = (semantic_projection.copy(), len(self.projection_cache))
self.memory.store(text_hash, semantic_projection, text)

# Ограничение размера кэша
if len(self.projection_cache) > 1000:
# Удаляем самые старые записи
oldest_key = min(self.projection_cache.items(),
key=lambda x: x[1][1])[0]
del self.projection_cache[oldest_key]

return semantic_projection

def _hash_text(self, text: str, context: Optional[str] = None) -> int:
"""Семантический хэш текста с учетом контекста"""
combined = text + (context or "")
# Используем частотные характеристики для хэширования
bytes_repr = combined.encode('utf-8')
hash_int = int(hashlib.sha256(bytes_repr).hexdigest()[:16], 16)
return hash_int % (2**32)

def _project_chars_to_sinusoids(self, text: str) -> np.ndarray:
"""Проекция символов на синусоидные частоты"""
vectors = []

for i, char in enumerate(text):
# Каждый символ → уникальная комбинация частот
char_code = ord(char)

# Синусоидная кодировка: разные частоты для разных битов символа
char_vector = np.zeros(self.space_dim)

for j in range(min(16, self.space_dim)):  # Используем 16 бит кода символа
bit = (char_code >> j) & 1
freq = 1.0 + j * 0.3
phase = i * 0.01  # Позиция в тексте влияет на фазу

# Синусоидная проекция бита
if bit:
char_vector += np.sin(freq * np.arange(self.space_dim) + phase)
else:
char_vector += np.cos(freq * np.arange(self.space_dim) + phase) * 0.5

vectors.append(char_vector)

return np.array(vectors)

def _adaptive_sequence_convolution(self, char_vectors: np.ndarray) -> np.ndarray:
"""Адаптивная свертка последовательности с вниманием к паттернам"""
if len(char_vectors) == 0:
return np.zeros(self.space_dim)

# Синусоидные веса для разных позиций
seq_len = len(char_vectors)
position_weights = np.sin(np.linspace(0, np.pi * 2, seq_len))

# Взвешенная сумма с позиционным кодированием
result = np.zeros(self.space_dim)

for i, (vec, weight) in enumerate(zip(char_vectors, position_weights)):
# Частотная модуляция в зависимости от позиции
freq_mod = 1.0 + i * 0.02
modulated = vec * np.sin(freq_mod * np.arange(self.space_dim))

# Добавляем с весом
result += modulated * weight

# Нормализация
norm = np.linalg.norm(result)
if norm > 0:
result = result / norm

return result

def semantic_similarity(self, vec1: np.ndarray, vec2: np.ndarray) -> float:
"""Косинусная схожесть с синусоидной коррекцией"""
cosine_sim = np.dot(vec1, vec2) / (
np.linalg.norm(vec1) * np.linalg.norm(vec2) + 1e-8
)

# Синусоидная коррекция для нелинейного сходства
corrected = np.sin(cosine_sim * np.pi / 2)
return float(corrected)

def create_context_vector(self, texts: List[str]) -> np.ndarray:
"""Создание контекстного вектора из нескольких текстов"""
if not texts:
return np.zeros(self.space_dim)

# Агрегация семантических векторов с адаптивными весами
vectors = [self.text_to_semantic_vector(t, None, temperature=0.3)
for t in texts]

# Синусоидное взвешивание: более поздние тексты имеют другую фазу
weights = np.sin(np.linspace(0, np.pi, len(vectors)))
weights = weights / np.sum(weights)

result = np.zeros(self.space_dim)
for vec, w in zip(vectors, weights):
result += vec * w

return result / np.linalg.norm(result)

def adapt_to_domain(self, domain_texts: List[str], learning_rate: float = 0.1):
"""Адаптация энкодера под конкретную предметную область"""
domain_vector = self.create_context_vector(domain_texts)

# Адаптация осей под домен
for axis in self.axes:
# Сдвигаем оси в направлении доменного вектора
similarity = np.dot(axis.base_vector, domain_vector)
adjustment = domain_vector * similarity * learning_rate

# Плавное обновление
axis.base_vector = axis.base_vector * 0.9 + adjustment * 0.1
axis.base_vector = axis.base_vector / np.linalg.norm(axis.base_vector)

# Адаптация веса оси
axis.weight *= (1.0 + similarity * learning_rate * 0.5)

class AdaptiveSemanticMemory:
"""Адаптивная семантическая память"""

def __init__(self, capacity: int = 10000):
self.capacity = capacity
self.memory = {}  # hash -> (vector, text, access_count, timestamp)
self.access_counter = 0

# KD-дерево для быстрого поиска по семантической близости
self.kdtree = None
self.vectors_for_tree = []
self.hashes_for_tree = []

def store(self, text_hash: int, vector: np.ndarray, text: str):
"""Сохранение в памяти"""
self.memory[text_hash] = {
'vector': vector.copy(),
'text': text,
'access_count': 1,
'timestamp': self.access_counter
}
self.access_counter += 1

# Обновление KD-дерева при достижении порога
if len(self.memory) % 100 == 0:
self._rebuild_kdtree()

# Очистка старых записей при переполнении
if len(self.memory) > self.capacity:
self._evict_oldest()

def find_similar(self, query_vector: np.ndarray,
threshold: float = 0.7) -> List[Tuple[str, float]]:
"""Поиск семантически похожих текстов"""
if not self.memory or self.kdtree is None:
return []

# Поиск по KD-дереву
distances, indices = self.kdtree.query(
query_vector,
k=min(10, len(self.vectors_for_tree))
)

results = []
for dist, idx in zip(distances, indices):
if dist < threshold:
text_hash = self.hashes_for_tree[idx]
entry = self.memory[text_hash]
similarity = 1.0 - dist  # преобразование расстояния в схожесть
results.append((entry['text'], similarity))

return sorted(results, key=lambda x: x[1], reverse=True)

def _rebuild_kdtree(self):
"""Перестроение KD-дерева для поиска"""
if not self.memory:
return

self.vectors_for_tree = []
self.hashes_for_tree = []

for text_hash, entry in self.memory.items():
self.vectors_for_tree.append(entry['vector'])
self.hashes_for_tree.append(text_hash)

if self.vectors_for_tree:
self.kdtree = KDTree(self.vectors_for_tree)

def _evict_oldest(self):
"""Удаление наименее используемых записей"""
# Сортировка по частоте использования и времени
entries = list(self.memory.items())
entries.sort(key=lambda x: (
x[1]['access_count'] * 0.3 +
x[1]['timestamp'] * 0.7
))

# Удаляем 10% старых записей
to_remove = int(len(entries) * 0.1)
for text_hash, _ in entries[:to_remove]:
del self.memory[text_hash]

class ContextTransformer:
"""Трансформация семантического пространства под контекст"""

def __init__(self, space_dim: int):
self.space_dim = space_dim

# Матрица трансформации, инициализированная синусоидными паттернами
self.transformation_matrix = self._create_sinusoidal_transformation()

def _create_sinusoidal_transformation(self) -> np.ndarray:
"""Создание синусоидной матрицы трансформации"""
matrix = np.zeros((self.space_dim, self.space_dim))

for i in range(self.space_dim):
for j in range(self.space_dim):
# Синусоидная связь между измерениями
freq = 1.0 + (i * j) % 7 * 0.3
matrix[i, j] = np.sin(freq * (i + j) * 0.1)

# Нормализация
for i in range(self.space_dim):
norm = np.linalg.norm(matrix[i, :])
if norm > 0:
matrix[i, :] /= norm

return matrix

def transform(self, vector: np.ndarray, context_vector: np.ndarray) -> np.ndarray:
"""Трансформация вектора под контекст"""
# Взвешенная комбинация с контекстным вектором
context_strength = np.linalg.norm(context_vector)
if context_strength > 0:
# Динамическая матрица трансформации на основе контекста
context_matrix = np.outer(context_vector, vector)
context_matrix = context_matrix / np.max(np.abs(context_matrix) + 1e-8)

# Применение трансформации
transformed = vector + np.dot(context_matrix, vector) * 0.3
else:
transformed = vector

# Применение базовой синусоидной трансформации
result = np.dot(self.transformation_matrix, transformed)

# Сохранение нормы
original_norm = np.linalg.norm(vector)
current_norm = np.linalg.norm(result)
if current_norm > 0:
result = result / current_norm * original_norm

return result

# ========== ИНТЕГРАЦИЯ С СИНУСОИДНОЙ СЕТЬЮ ==========

class CSEForSinusoidalNetwork:
"""Интерфейс CSE для синусоидной нейросети на синапсах"""

def __init__(self):
self.encoder = CSEEncoder(space_dimensions=128, n_axes=32)

# Буфер для потоковой обработки
self.stream_buffer = []
self.context_window = []

def stream_encode(self, text_stream: List[str]) -> List[np.ndarray]:
"""
Потоковое кодирование для синусоидной сети

Args:
text_stream: поток текстовых фрагментов

Returns:
Поток семантических векторов для немедленной подачи в сеть
"""
vectors = []

for text in text_stream:
# Обновление контекстного окна
self.context_window.append(text)
if len(self.context_window) > 5:
self.context_window.pop(0)

# Создание контекста из предыдущих фрагментов
context = " ".join(self.context_window[:-1]) if len(self.context_window) > 1 else None

# Кодирование с учетом контекста
vector = self.encoder.text_to_semantic_vector(
text,
context=context,
temperature=0.1  # Низкая температура для стабильности потока
)

vectors.append(vector)

# Адаптация к часто встречающимся паттернам
if len(self.stream_buffer) > 100:
self._adapt_to_stream_patterns()

return vectors

def _adapt_to_stream_patterns(self):
"""Адаптация к паттернам в потоке данных"""
if len(self.stream_buffer) < 50:
return

# Анализ частотности паттернов
from collections import Counter
pattern_counter = Counter(self.stream_buffer[-100:])

# Адаптация под частые паттерны
frequent_patterns = [p for p, c in pattern_counter.items() if c > 3]
if frequent_patterns:
self.encoder.adapt_to_domain(frequent_patterns, learning_rate=0.05)

def batch_encode_for_training(self, texts: List[str],
batch_size: int = 32) -> np.ndarray:
"""Пакетное кодирование для обучения сети"""
vectors = []

for i in range(0, len(texts), batch_size):
batch = texts[i:i+batch_size]

# Параллельное кодирование (можно распараллелить)
for text in batch:
vector = self.encoder.text_to_semantic_vector(
text,
context=None,
temperature=0.05  # Минимальная вариативность для обучения
)
vectors.append(vector)

return np.array(vectors)

def decode_to_nearest_text(self, vector: np.ndarray,
n_candidates: int = 3) -> List[str]:
"""
Декодирование вектора в ближайшие текстовые представления

Важно: CSE не является биективным кодированием!
Возвращаем наиболее семантически близкие известные тексты
"""
# Поиск в памяти энкодера
similar = self.encoder.memory.find_similar(vector, threshold=0.6)

if similar:
return [text for text, _ in similar[:n_candidates]]

# Если ничего не найдено, генерируем описательную строку
return [self._vector_to_description(vector)]

def _vector_to_description(self, vector: np.ndarray) -> str:
"""Преобразование вектора в текстовое описание"""
# Анализ активаций по осям
axis_activations = []

for axis in self.encoder.axes:
activation = np.dot(vector, axis.base_vector) * axis.weight
if abs(activation) > 0.3:
axis_activations.append((axis.name, activation))

# Сортировка по силе активации
axis_activations.sort(key=lambda x: abs(x[1]), reverse=True)

# Создание описания
if axis_activations:
top_axes = axis_activations[:3]
descriptors = [f"{name}({act:.2f})" for name, act in top_axes]
return f"Vector[{','.join(descriptors)}]"

return f"SemanticVector[norm={np.linalg.norm(vector):.2f}]"

# ========== ПРИМЕР ИСПОЛЬЗОВАНИЯ ==========

def example_usage():
"""Пример использования CSE"""

# Инициализация
cse = CSEForSinusoidalNetwork()

# Потоковая обработка (как в синусоидной сети)
stream = [
"Привет, как дела?",
"У меня всё хорошо, спасибо!",
"Что нового в проекте?",
"Завершили модуль семантического кодирования."
]

print("Потоковое кодирование:")
vectors = cse.stream_encode(stream)

for text, vec in zip(stream, vectors):
print(f"\nТекст: {text}")
print(f"Размер вектора: {vec.shape}")
print(f"Норма: {np.linalg.norm(vec):.2f}")

# Поиск похожих в памяти
similar = cse.encoder.memory.find_similar(vec)
if similar:
print(f"Похожие в памяти: {[t[:30] for t, _ in similar[:2]]}")

# Семантическое сравнение
print("\n\nСемантическое сравнение:")
vec1 = cse.encoder.text_to_semantic_vector("искусственный интеллект")
vec2 = cse.encoder.text_to_semantic_vector("нейронная сеть")
vec3 = cse.encoder.text_to_semantic_vector("кофеварка")

sim12 = cse.encoder.semantic_similarity(vec1, vec2)
sim13 = cse.encoder.semantic_similarity(vec1, vec3)

print(f"Схожесть 'ИИ' и 'нейросеть': {sim12:.3f}")
print(f"Схожесть 'ИИ' и 'кофеварка': {sim13:.3f}")

# Адаптация под домен
print("\n\nАдаптация под технический домен:")
tech_texts = [
"алгоритм машинного обучения",
"глубокое обучение с подкреплением",
"трансформер архитектура",
"обработка естественного языка"
]

cse.encoder.adapt_to_domain(tech_texts, learning_rate=0.1)

# Проверка адаптации
tech_vec = cse.encoder.text_to_semantic_vector("внимание механизм")
similar_tech = cse.encoder.memory.find_similar(tech_vec)
print(f"Для 'внимание механизм' найдено: {len(similar_tech)} похожих")

if __name__ == "__main__":
example_usage()
