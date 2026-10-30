#!/usr/bin/env python3
"""
Быстрый тест всей системы SSN
"""

print("🧪 Тестирование Sinusoidal Synaptic Network...")
print("=" * 60)

# Тест импортов
try:
from cse_encoder import CSEForSinusoidalNetwork
from synaptic_core import SinusoidalSynapticCore
from sample import StreamFlowSampler

print("✅ Импорты успешны")

# Быстрая инициализация
print("\n🔄 Инициализация CSE...")
cse = CSEForSinusoidalNetwork()

print("🔄 Инициализация SSN...")
ssn = SinusoidalSynapticCore(
input_dim=128,
hidden_dims=[256, 128],
output_dim=128,
n_synapses_per_block=64
)

print("🔄 Инициализация StreamFlow...")
sampler = StreamFlowSampler(ssn, cse, flow_dim=128)

# Быстрый тест кодирования
print("\n📝 Тест семантического кодирования...")
test_text = "Привет, это тест синусоидной сети"
vector = cse.encoder.text_to_semantic_vector(test_text)
print(f"   Текст: {test_text}")
print(f"   Размер вектора: {vector.shape}")
print(f"   Норма вектора: {np.linalg.norm(vector):.3f}")

# Быстрый тест обработки
print("\n⚡ Тест потоковой обработки...")
output = ssn.stream_forward(vector)
print(f"   Вход: {vector.shape}")
print(f"   Выход: {output.shape}")
print(f"   Энергия: {np.linalg.norm(output):.3f}")

print("\n🎉 Все системы работают!")
print("=" * 60)

except ImportError as e:
print(f"❌ Ошибка импорта: {e}")
except Exception as e:
print(f"❌ Ошибка выполнения: {e}")

import numpy as np
