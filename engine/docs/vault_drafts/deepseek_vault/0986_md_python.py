"""
Sinusoidal Synaptic Network (SSN)
Полностью заменяет трансформерную архитектуру
"""

import numpy as np
import tensorflow as tf
from typing import Dict, List, Optional, Tuple, Union
from dataclasses import dataclass
import math

# ========== ОСНОВНЫЕ СТРУКТУРЫ ДАННЫХ ==========

@dataclass
class Synapse:
"""Единичный синапс - атомарная вычислительная единица"""
weight: float          # 2 байта (float16)
bias: float           # 2 байта (float16)
func_id: int          # 0.5 байта (uint4) - идентификатор функции
meta: np.ndarray      # 4 байта - метаданные для адаптации

def __post_init__(self):
self.activation_count = 0
self.last_energy = 0.0
self.adaptation_rate = 0.01

@dataclass
class SynapticBlock:
"""Блок синапсов - функциональная группа"""
synapses: List[Synapse]           # Синапсы в блоке
block_type: str                   # 'linear', 'sinusoidal', 'decoder', 'attention'
input_dim: int                    # Размерность входа
output_dim: int                   # Размерность выхода
learning_rate: float = 0.001      # Скорость адаптации блока

def __post_init__(self):
# Инициализация через синусоидные частоты
self.frequency_matrix = self._init_frequency_matrix()
self.phase_shift = np.random.randn(self.output_dim) * 0.1
self.energy_history = []

class SynapticLayer:
"""Слой из нескольких блоков"""
def __init__(self, blocks: List[SynapticBlock], name: str = "synaptic_layer"):
self.blocks = blocks
self.name = name
self.global_adaptation_rate = 0.01

# Метрики слоя для самонаблюдения
self.layer_energy = 0.0
self.activation_map = {}
self.error_signal = 0.0

# ========== ЯДРО SSN ==========

class SinusoidalSynapticCore:
"""Ядро синусоидной синаптической сети"""

def __init__(self,
input_dim: int = 256,
hidden_dims: List[int] = [512, 512],
output_dim: int = 256,
n_synapses_per_block: int = 256,
use_adaptive_learning: bool = True):

self.input_dim = input_dim
self.output_dim = output_dim
self.n_synapses_per_block = n_synapses_per_block
self.use_adaptive_learning = use_adaptive_learning

# Потоковые буферы
self.state_buffer = []           # Буфер состояний
self.error_buffer = []           # Буфер ошибок
self.energy_buffer = []          # Буфер энергии

# Создание архитектуры сети
self.layers = self._build_network(hidden_dims)

# Контур самонаблюдения
self.observation_circuit = ObservationCircuit()

# Динамические параметры
self.flow_temperature = 0.1      # "Температура" потока (вариативность)
self.resonance_factor = 0.5      # Фактор резонанса (усиление паттернов)
self.entropy_target = 0.3        # Целевая энтропия системы

def _build_network(self, hidden_dims: List[int]) -> List[SynapticLayer]:
"""Построение иерархии синаптических слоев"""
layers = []

# Входной слой
input_blocks = [
self._create_block('sinusoidal', self.input_dim, hidden_dims[0]),
self._create_block('linear', self.input_dim, hidden_dims[0] // 4)
]
layers.append(SynapticLayer(input_blocks, "input_layer"))

# Скрытые слои
for i, (in_dim, out_dim) in enumerate(zip(hidden_dims[:-1], hidden_dims[1:])):
blocks = [
self._create_block('sinusoidal', in_dim, out_dim),
self._create_block('attention', in_dim, out_dim // 2),
self._create_block('linear', in_dim, out_dim // 4)
]
layers.append(SynapticLayer(blocks, f"hidden_layer_{i}"))

# Выходной слой
output_blocks = [
self._create_block('decoder', hidden_dims[-1], self.output_dim),
self._create_block('linear', hidden_dims[-1], self.output_dim // 2)
]
layers.append(SynapticLayer(output_blocks, "output_layer"))

return layers

def _create_block(self, block_type: str, input_dim: int, output_dim: int) -> SynapticBlock:
"""Создание блока синапсов"""
synapses = []

# Создание синапсов в зависимости от типа блока
n_synapses = min(self.n_synapses_per_block, output_dim * 4)

for i in range(n_synapses):
# Инициализация весов через синусоидное распределение
freq = 1.0 + (i % 16) * 0.3
phase = (i // 16) * 0.1

# Вес как функция синуса от частоты
weight = np.sin(freq * phase) * 0.5

# Смещение через косинус
bias = np.cos(freq * phase) * 0.2

# Выбор функции активации
if block_type == 'sinusoidal':
func_id = 0  # Синусоидная
elif block_type == 'attention':
func_id = 1  # Attention-like
elif block_type == 'decoder':
func_id = 2  # Декодер
else:
func_id = 3  # Линейная

# Метаданные для адаптации
meta = np.array([freq, phase, 0.0, 0.0])  # частота, фаза, энергия, адаптация

synapse = Synapse(
weight=float(weight),
bias=float(bias),
func_id=func_id,
meta=meta
)
synapses.append(synapse)

return SynapticBlock(
synapses=synapses,
block_type=block_type,
input_dim=input_dim,
output_dim=output_dim
)

def _apply_synaptic_function(self, x: np.ndarray, synapse: Synapse) -> float:
"""Применение функции синапса к входу"""
# Базовое преобразование
z = synapse.weight * np.dot(x, x) + synapse.bias  # Скалярное произведение

if synapse.func_id == 0:  # Синусоидная
freq = synapse.meta[0]
return np.sin(freq * z)

elif synapse.func_id == 1:  # Attention-like
# Упрощенное подобие внимания
attention = np.tanh(z * 0.5)
return attention

elif synapse.func_id == 2:  # Декодер
# Софтмаксоподобная функция для декодирования
exp_z = np.exp(z - np.max(z))
return exp_z / (np.sum(exp_z) + 1e-8)

else:  # Линейная
return z * 0.1  # Слабый линейный отклик

def _process_block(self, x: np.ndarray, block: SynapticBlock) -> np.ndarray:
"""Обработка входа через блок синапсов"""
outputs = []
block_energy = 0.0

# Параллельная обработка через синапсы
for synapse in block.synapses:
# Применение функции синапса
output = self._apply_synaptic_function(x, synapse)
outputs.append(output)

# Обновление метрик синапса
synapse.activation_count += 1
energy = abs(output)
synapse.last_energy = energy
block_energy += energy

# Адаптация веса через локальное правило Хебба
if self.use_adaptive_learning:
self._adapt_synapse(synapse, x, output)

# Нормализация выхода блока
outputs = np.array(outputs)

# Применение частотной матрицы для синусоидных блоков
if block.block_type == 'sinusoidal':
outputs = outputs * block.frequency_matrix[:len(outputs)]
outputs = outputs + block.phase_shift[:len(outputs)] * 0.1

# Сохранение энергии блока
block.energy_history.append(block_energy / len(block.synapses))
if len(block.energy_history) > 100:
block.energy_history.pop(0)

return outputs[:block.output_dim]  # Обрезка до нужной размерности

def _adapt_synapse(self, synapse: Synapse, x: np.ndarray, output: float):
"""Локальная адаптация синапса"""
# Правило Хебба с регуляризацией
hebbian_update = output * np.mean(x) * 0.01

# Анти-Хебб для стабильности
anti_hebbian = -synapse.weight * 0.001

# Обновление с адаптивной скоростью
update = hebbian_update + anti_hebbian
synapse.weight += update * synapse.adaptation_rate

# Адаптация скорости обучения
if synapse.activation_count % 100 == 0:
# Если энергия высокая и стабильная - уменьшаем скорость
if synapse.last_energy > 0.8 and abs(update) < 0.01:
synapse.adaptation_rate *= 0.99
else:
synapse.adaptation_rate *= 1.01

# Ограничение скорости
synapse.adaptation_rate = np.clip(synapse.adaptation_rate, 1e-6, 0.1)

def stream_forward(self, x: np.ndarray, context: Optional[np.ndarray] = None) -> np.ndarray:
"""
Прямой потоковый проход

Args:
x: входной вектор состояния
context: контекстный вектор (опционально)

Returns:
Выходной вектор состояния
"""
# Сохранение в буфер состояния
self.state_buffer.append(x.copy())
if len(self.state_buffer) > 1000:
self.state_buffer.pop(0)

# Начальное состояние
current_state = x

# Проход по всем слоям
layer_outputs = []
for layer in self.layers:
layer_output = np.zeros(layer.blocks[0].output_dim)
layer_energy = 0.0

# Параллельная обработка блоками слоя
for block in layer.blocks:
# Обработка через блок
block_output = self._process_block(current_state, block)

# Агрегация выходов блоков (конкатенация)
if len(layer_output) < len(block_output):
layer_output = np.concatenate([
layer_output,
block_output[:len(block_output) - len(layer_output)]
])
else:
layer_output[:len(block_output)] += block_output

layer_energy += np.mean(np.abs(block_output))

# Нормализация выхода слоя
layer_norm = np.linalg.norm(layer_output)
if layer_norm > 0:
layer_output = layer_output / layer_norm

# Обновление метрик слоя
layer.layer_energy = layer_energy / len(layer.blocks)
layer_outputs.append(layer_output)

# Переход к следующему слою
current_state = layer_output

# Контур самонаблюдения
self._observation_step(layer_outputs)

# Выход - результат последнего слоя
output = layer_outputs[-1]

# Добавление резонанса из буфера состояний
if len(self.state_buffer) > 10:
# Резонанс с предыдущими состояниями
resonance = np.mean(self.state_buffer[-10:], axis=0)
output = output * (1 - self.resonance_factor) + resonance * self.resonance_factor

return output

def _observation_step(self, layer_outputs: List[np.ndarray]):
"""Шаг контура самонаблюдения"""
# Вычисление общей энергии системы
total_energy = np.mean([np.linalg.norm(out) for out in layer_outputs])
self.energy_buffer.append(total_energy)

# Адаптация глобальных параметров
if len(self.energy_buffer) > 100:
energy_std = np.std(self.energy_buffer[-100:])

# Регулировка температуры потока
if energy_std < 0.05:  # Слишком стабильно
self.flow_temperature = min(0.3, self.flow_temperature * 1.1)
elif energy_std > 0.2:  # Слишком хаотично
self.flow_temperature = max(0.01, self.flow_temperature * 0.9)

# Вычисление энтропии системы
entropy = self._compute_system_entropy()

# Подстройка к целевой энтропии
entropy_error = self.entropy_target - entropy
self.resonance_factor += entropy_error * 0.01
self.resonance_factor = np.clip(self.resonance_factor, 0.1, 0.9)

def _compute_system_entropy(self) -> float:
"""Вычисление энтропии системы по буферу состояний"""
if len(self.state_buffer) < 2:
return 0.0

# Простая оценка энтропии через вариацию состояний
recent_states = np.array(self.state_buffer[-100:])
cov_matrix = np.cov(recent_states.T)

# Энтропия как логарифм определителя ковариационной матрицы
try:
entropy = 0.5 * np.log(np.linalg.det(cov_matrix + np.eye(cov_matrix.shape[0]) * 1e-6))
except:
entropy = 0.0

return float(entropy)

def train_stream(self,
input_stream: List[np.ndarray],
target_stream: Optional[List[np.ndarray]] = None,
n_iterations: int = 1000):
"""
Потоковое обучение сети

Args:
input_stream: поток входных векторов
target_stream: поток целевых векторов (опционально)
n_iterations: число итераций
"""
print("Начало потокового обучения SSN...")

for iteration in range(n_iterations):
# Выбор случайного входа из потока
idx = np.random.randint(0, len(input_stream))
x = input_stream[idx]

# Прямой проход
y_pred = self.stream_forward(x)

# Вычисление ошибки (если есть целевой поток)
if target_stream is not None:
y_target = target_stream[idx]
error = np.mean((y_pred - y_target) ** 2)
self.error_buffer.append(error)

# Адаптация на основе ошибки
self._adapt_to_error(error, y_pred, y_target)

# Логгирование
if iteration % 100 == 0:
energy = np.mean(self.energy_buffer[-100:] if self.energy_buffer else [0])
entropy = self._compute_system_entropy()

print(f"Iter {iteration}: Energy={energy:.4f}, "
f"Entropy={entropy:.4f}, "
f"Temp={self.flow_temperature:.3f}")

def _adapt_to_error(self, error: float, prediction: np.ndarray, target: np.ndarray):
"""Адаптация сети на основе ошибки"""
# Распространение сигнала ошибки обратно по слоям
error_signal = target - prediction

# Адаптация выходного слоя
output_layer = self.layers[-1]
for block in output_layer.blocks:
# Усиление синапсов, способствовавших правильному ответу
for synapse in block.synapses:
if synapse.last_energy > 0.5:
# Если синапс был активен и ошибка мала - усиливаем
if error < 0.1:
synapse.weight *= 1.01
# Если ошибка велика - ослабляем
else:
synapse.weight *= 0.99

# Ограничение весов
synapse.weight = np.clip(synapse.weight, -5.0, 5.0)

# Глобальная адаптация скорости обучения
if len(self.error_buffer) > 100:
recent_errors = self.error_buffer[-100:]
error_trend = np.mean(np.diff(recent_errors))

if error_trend < 0:  # Ошибка уменьшается
# Ускоряем адаптацию
for layer in self.layers:
for block in layer.blocks:
block.learning_rate = min(0.01, block.learning_rate * 1.01)
else:  # Ошибка растет
# Замедляем адаптацию
for layer in self.layers:
for block in layer.blocks:
block.learning_rate = max(1e-6, block.learning_rate * 0.99)

# ========== СПЕЦИАЛЬНЫЕ БЛОКИ ==========

class StateAttentionBlock(SynapticBlock):
"""Блок внимания к состоянию (State Attention)"""

def __init__(self, input_dim: int, output_dim: int, n_heads: int = 4):
super().__init__([], 'attention', input_dim, output_dim)
self.n_heads = n_heads
self.attention_weights = np.random.randn(n_heads, input_dim, input_dim) * 0.01

def process(self, x: np.ndarray, state_memory: List[np.ndarray]) -> np.ndarray:
"""Обработка с вниманием к предыдущим состояниям"""
if not state_memory:
return x[:self.output_dim]

# Создание запросов, ключей и значений из текущего состояния
Q = self._project(x, 'query')

# Ключи и значения из памяти состояний
memory_matrix = np.array(state_memory[-10:])  # Последние 10 состояний
K = self._project(memory_matrix, 'key')
V = self._project(memory_matrix, 'value')

# Многоголовое внимание
head_outputs = []
for h in range(self.n_heads):
# Внимание между текущим состоянием и памятью
scores = np.dot(Q[h], K[h].T) / np.sqrt(self.input_dim)
attention = softmax(scores)

# Взвешенная сумма значений
head_output = np.dot(attention, V[h])
head_outputs.append(head_output)

# Конкатенация и проекция
combined = np.concatenate(head_outputs, axis=-1)
output = self._project(combined, 'output')[:self.output_dim]

return output

def _project(self, x: np.ndarray, proj_type: str) -> np.ndarray:
"""Проекция для внимания"""
if proj_type == 'query':
idx = 0
elif proj_type == 'key':
idx = 1
elif proj_type == 'value':
idx = 2
else:  # output
idx = 3

# Упрощенная линейная проекция
return x @ self.attention_weights[idx % self.n_heads]

class SinusoidalOscillatorBlock(SynapticBlock):
"""Синусоидный осцилляторный блок для временных зависимостей"""

def __init__(self, input_dim: int, output_dim: int, base_frequency: float = 1.0):
super().__init__([], 'sinusoidal', input_dim, output_dim)
self.base_frequency = base_frequency
self.time_step = 0
self.phase_accumulator = np.zeros(output_dim)

# Инициализация частот для каждого выхода
self.frequencies = np.logspace(
np.log10(base_frequency),
np.log10(base_frequency * 10),
output_dim
)

# Фазовые сдвиги
self.phases = np.random.rand(output_dim) * 2 * np.pi

def process(self, x: np.ndarray) -> np.ndarray:
"""Обработка с временным осциллятором"""
self.time_step += 1

# Модуляция входного сигнала осцилляторами
modulated = np.zeros(self.output_dim)

for i in range(self.output_dim):
# Осциллятор с нарастающей фазой
self.phase_accumulator[i] += self.frequencies[i] * 0.01

# Синусоидная модуляция
oscillator = np.sin(self.phase_accumulator[i] + self.phases[i])

# Взвешивание входом
if i < len(x):
modulated[i] = x[i] * oscillator
else:
modulated[i] = oscillator

# Добавление гармоник
harmonics = np.sin(self.phase_accumulator * 2) * 0.3
modulated += harmonics

return modulated

# ========== КОНТУР САМОНАБЛЮДЕНИЯ ==========

class ObservationCircuit:
"""Контур самонаблюдения и адаптации"""

def __init__(self):
self.metrics_history = []
self.adaptation_signals = {}
self.stability_threshold = 0.1

def observe(self,
layers: List[SynapticLayer],
error_signals: List[float],
energy_levels: List[float]):
"""Наблюдение за состоянием сети"""

# Сбор метрик
metrics = {
'layer_energies': [l.layer_energy for l in layers],
'error_magnitude': np.mean(np.abs(error_signals)) if error_signals else 0.0,
'energy_variance': np.var(energy_levels) if energy_levels else 0.0,
'activation_density': self._compute_activation_density(layers),
'adaptation_rates': [b.learning_rate for l in layers for b in l.blocks]
}

self.metrics_history.append(metrics)
if len(self.metrics_history) > 1000:
self.metrics_history.pop(0)

# Анализ стабильности
self._analyze_stability(metrics, layers)

# Генерация сигналов адаптации
self._generate_adaptation_signals(metrics, layers)

def _compute_activation_density(self, layers: List[SynapticLayer]) -> float:
"""Вычисление плотности активации сети"""
total_activations = 0
total_synapses = 0

for layer in layers:
for block in layer.blocks:
for synapse in block.synapses:
total_synapses += 1
if synapse.last_energy > 0.1:  # Порог активации
total_activations += 1

return total_activations / total_synapses if total_synapses > 0 else 0.0

def _analyze_stability(self, metrics: Dict, layers: List[SynapticLayer]):
"""Анализ стабильности системы"""
if len(self.metrics_history) < 10:
return

# Вычисление тренда ошибки
recent_errors = [m['error_magnitude'] for m in self.metrics_history[-10:]]
error_trend = np.polyfit(range(len(recent_errors)), recent_errors, 1)[0]

# Регулировка скоростей обучения на основе стабильности
if abs(error_trend) < self.stability_threshold:
# Система стабильна - можно увеличить скорость обучения
self._adjust_learning_rates(layers, multiplier=1.01)
else:
# Система нестабильна - уменьшаем скорость
self._adjust_learning_rates(layers, multiplier=0.99)

def _adjust_learning_rates(self, layers: List[SynapticLayer], multiplier: float):
"""Регулировка скоростей обучения всех блоков"""
for layer in layers:
for block in layer.blocks:
block.learning_rate *= multiplier
block.learning_rate = np.clip(block.learning_rate, 1e-6, 0.1)

def _generate_adaptation_signals(self, metrics: Dict, layers: List[SynapticLayer]):
"""Генерация сигналов для адаптации сети"""
signals = {}

# Сигнал на основе плотности активации
activation_density = metrics['activation_density']
if activation_density < 0.1:  # Слишком мало активаций
signals['increase_sensitivity'] = 0.1
elif activation_density > 0.9:  # Слишком много активаций
signals['decrease_sensitivity'] = 0.1

# Сигнал на основе дисперсии энергии
energy_var = metrics['energy_variance']
if energy_var > 0.2:  # Высокая дисперсия - нестабильность
signals['stabilize'] = energy_var * 0.5

self.adaptation_signals = signals

# ========== УТИЛИТЫ ==========

def softmax(x: np.ndarray, axis: int = -1) -> np.ndarray:
"""Стабильный softmax"""
x = x - np.max(x, axis=axis, keepdims=True)
exp_x = np.exp(x)
return exp_x / np.sum(exp_x, axis=axis, keepdims=True)

def create_input_pipeline(vocab_size: int = 50000,
embedding_dim: int = 256,
use_cse: bool = True):
"""Создание входного конвейера"""
if use_cse:
# Использование Continuous Semantic Encoding
from cse_encoder import CSEForSinusoidalNetwork
return CSEForSinusoidalNetwork()
else:
# Простой эмбеддинг (для обратной совместимости)
class SimpleEmbedding:
def __init__(self, vocab_size, embedding_dim):
self.embedding_matrix = np.random.randn(vocab_size, embedding_dim) * 0.01

def encode(self, token_ids):
return np.dot(token_ids, self.embedding_matrix)

return SimpleEmbedding(vocab_size, embedding_dim)

# ========== ПРИМЕР ИСПОЛЬЗОВАНИЯ ==========

def example_usage():
"""Пример использования SSN"""

# Создание сети
ssn = SinusoidalSynapticCore(
input_dim=256,
hidden_dims=[512, 512, 256],
output_dim=256,
n_synapses_per_block=128
)

# Создание тестовых данных
print("Генерация тестовых данных...")
n_samples = 1000
input_stream = [np.random.randn(256) * 0.5 for _ in range(n_samples)]
target_stream = [np.sin(x * 2) * 0.3 for x in input_stream]  # Простая целевая функция

# Обучение
print("Запуск потокового обучения...")
ssn.train_stream(input_stream, target_stream, n_iterations=500)

# Тестирование
print("\nТестирование...")
test_input = np.random.randn(256) * 0.5
output = ssn.stream_forward(test_input)

print(f"Входная норма: {np.linalg.norm(test_input):.4f}")
print(f"Выходная норма: {np.linalg.norm(output):.4f}")
print(f"Энтропия системы: {ssn._compute_system_entropy():.4f}")
print(f"Температура потока: {ssn.flow_temperature:.4f}")

# Анализ метрик
print("\nМетрики слоев:")
for i, layer in enumerate(ssn.layers):
print(f"Слой {i} ({layer.name}): энергия={layer.layer_energy:.4f}")

return ssn

if __name__ == "__main__":
# Запуск примера
trained_ssn = example_usage()

# Сохранение модели
print("\nМодель SSN готова к использованию!")
