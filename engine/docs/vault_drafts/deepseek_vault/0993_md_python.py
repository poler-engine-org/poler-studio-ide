#!/usr/bin/env python3
"""
Универсальный загрузчик моделей SSN
"""

import pickle
import json
from pathlib import Path
from typing import Dict, Any, Optional
import numpy as np

from cse_encoder import CSEForSinusoidalNetwork
from synaptic_core import SinusoidalSynapticCore
from sample import StreamFlowSampler

class SSNModel:
"""
Универсальный класс для работы с моделями SSN
"""

def __init__(self, model_dir: str, auto_load: bool = True):
"""
Args:
model_dir: путь к директории модели
auto_load: автоматически загрузить компоненты
"""
self.model_dir = Path(model_dir)
self.is_loaded = False

# Компоненты модели
self.cse = None
self.cse_interface = None
self.ssn = None
self.sampler = None
self.config = None
self.metadata = None

if auto_load:
self.load()

def load(self) -> bool:
"""Загрузка всех компонентов модели"""
try:
print(f"📦 Загрузка модели из: {self.model_dir}")

# Загрузка конфигурации
config_file = self.model_dir / 'config.json'
if config_file.exists():
with open(config_file, 'r', encoding='utf-8') as f:
self.config = json.load(f)

# Загрузка метаданных
meta_file = self.model_dir / 'metadata.json'
if meta_file.exists():
with open(meta_file, 'r', encoding='utf-8') as f:
self.metadata = json.load(f)

# Загрузка CSE
cse_file = self.model_dir / 'cse_encoder.pkl'
if cse_file.exists():
with open(cse_file, 'rb') as f:
self.cse = pickle.load(f)

# Загрузка SSN
ssn_file = self.model_dir / 'ssn_core.pkl'
if ssn_file.exists():
with open(ssn_file, 'rb') as f:
self.ssn = pickle.load(f)

# Создание интерфейсов
self._create_interfaces()

self.is_loaded = True
print(f"✅ Модель успешно загружена")
print(f"   Размерность: {self.config['input_dim']}D")
print(f"   Синапсов: {self._count_synapses()}")

return True

except Exception as e:
print(f"❌ Ошибка загрузки модели: {e}")
return False

def _create_interfaces(self):
"""Создание интерфейсов для удобства"""
# CSE интерфейс
self.cse_interface = CSEForSinusoidalNetwork()
self.cse_interface.encoder = self.cse

# StreamFlow Sampler
if self.ssn and self.cse_interface:
self.sampler = StreamFlowSampler(
ssn_core=self.ssn,
cse_encoder=self.cse_interface,
flow_dim=self.config['input_dim']
)

def _count_synapses(self) -> int:
"""Подсчет общего количества синапсов"""
if not self.ssn or not hasattr(self.ssn, 'layers'):
return 0

total = 0
for layer in self.ssn.layers:
for block in layer.blocks:
total += len(block.synapses)
return total

def encode(self, text: str, temperature: float = 0.3) -> np.ndarray:
"""Кодирование текста в семантический вектор"""
if not self.is_loaded:
self.load()

return self.cse.encoder.text_to_semantic_vector(
text,
temperature=temperature
)

def process(self, vector: np.ndarray) -> np.ndarray:
"""Обработка вектора через синаптическую сеть"""
if not self.is_loaded:
self.load()

return self.ssn.stream_forward(vector)

def generate(self,
prompt: str,
max_length: int = 100,
creativity: float = 0.5) -> str:
"""
Генерация текста по промпту

Args:
prompt: начальный текст
max_length: максимальная длина в словах
creativity: креативность (0.0-1.0)
"""
if not self.is_loaded:
self.load()

# Кодирование промпта
prompt_vector = self.encode(prompt, temperature=creativity)

# Инициализация потока
flow_state = self.sampler.initialize_flow(initial_vector=prompt_vector)

# Настройка параметров
self.sampler.flow_params.creativity = creativity
self.sampler.flow_params.temperature = creativity * 0.5

# Генерация последовательности
sequence = self.sampler.flow_sequence(
n_steps=max_length,
initial_state=flow_state
)

# Декодирование
texts = self.sampler.decode_sequence(sequence, decode_mode='adaptive')

# Объединение результатов
result = self._combine_texts(texts)

return result

def _combine_texts(self, texts: list) -> str:
"""Объединение текстов в связный результат"""
if not texts:
return ""

result = texts[0]

for text in texts[1:]:
# Добавляем с учетом пунктуации
if result.endswith(('.', '!', '?')):
result += " " + text.capitalize()
else:
result += " " + text

return result

def chat(self,
message: str,
context: Optional[str] = None,
temperature: float = 0.3) -> str:
"""
Чат-режим с поддержкой контекста

Args:
message: сообщение пользователя
context: предыдущий контекст (опционально)
temperature: креативность ответа
"""
if not self.is_loaded:
self.load()

# Подготовка текста с контекстом
if context:
full_text = context + " " + message
else:
full_text = message

# Генерация ответа
response = self.generate(full_text, creativity=temperature)

# Удаление контекста из ответа если он там есть
if context and response.startswith(context):
response = response[len(context):].strip()

return response

def get_info(self) -> Dict[str, Any]:
"""Получение информации о модели"""
return {
'loaded': self.is_loaded,
'config': self.config,
'metadata': self.metadata,
'synapse_count': self._count_synapses(),
'dimensions': self.config['input_dim'] if self.config else None,
'model_dir': str(self.model_dir)
}

def load_ssn_model(model_name: str, models_dir: str = 'models') -> SSNModel:
"""
Загрузка модели по имени

Args:
model_name: имя модели (папка)
models_dir: корневая директория моделей

Returns:
Экземпляр SSNModel
"""
model_path = Path(models_dir) / model_name

if not model_path.exists():
# Попробуем найти в других местах
possible_paths = [
model_path,
Path(f"models/{model_name}"),
Path(f"../models/{model_name}"),
Path(f"./{model_name}")
]

for path in possible_paths:
if path.exists():
model_path = path
break
else:
raise FileNotFoundError(f"Модель '{model_name}' не найдена")

return SSNModel(str(model_path))

# ========== ФАСАДНЫЕ ФУНКЦИИ ==========

def create_model(model_name: str = "ssn_medium",
model_size: str = "medium",
models_dir: str = "models"):
"""
Создание новой модели (фасад для initialize_ssn.py)
"""
from initialize_ssn import SSNInitializer

initializer = SSNInitializer(models_dir)
return initializer.create_model(model_name, model_size)

def list_models(models_dir: str = "models"):
"""
Список доступных моделей
"""
from initialize_ssn import SSNInitializer

initializer = SSNInitializer(models_dir)
return initializer.list_models()

# ========== КОМАНДНАЯ СТРОКА ==========

def main():
"""Командная строка для загрузчика"""
import argparse

parser = argparse.ArgumentParser(description='SSN Model Loader')
parser.add_argument('model', nargs='?', help='Имя модели для загрузки')
parser.add_argument('--list', action='store_true', help='Список моделей')
parser.add_argument('--info', action='store_true', help='Информация о модели')
parser.add_argument('--test', action='store_true', help='Тестирование модели')
parser.add_argument('--prompt', type=str, help='Промпт для тестирования')

args = parser.parse_args()

if args.list:
print("📚 Доступные модели:")
print("=" * 60)

try:
models = list_models()
for model in models:
print(f"🔸 {model['name']} ({model['size']})")
print(f"   Размерность: {model['dimensions']}D")
print(f"   Создана: {model['created']}")
print()
except Exception as e:
print(f"❌ Ошибка: {e}")

elif args.model:
try:
print(f"🔄 Загрузка модели: {args.model}")
model = load_ssn_model(args.model)

if args.info:
info = model.get_info()
print(f"\n📊 Информация о модели:")
print(f"   Загружена: {'✅' if info['loaded'] else '❌'}")
print(f"   Размерность: {info['dimensions']}D")
print(f"   Синапсов: {info['synapse_count']:,}")
print(f"   Директория: {info['model_dir']}")

if args.test or args.prompt:
prompt = args.prompt or "Привет, расскажи о синаптических сетях"
print(f"\n🧪 Тестирование генерации:")
print(f"   Промпт: {prompt}")
print(f"   Результат: ", end='', flush=True)

response = model.generate(prompt, max_length=50, creativity=0.5)
print(f"{response}")

except Exception as e:
print(f"❌ Ошибка: {e}")

else:
print("""
🔧 SSN Model Loader
===================
Использование:
python ssn_loader.py [model_name]    # Загрузить модель
python ssn_loader.py --list          # Список моделей
python ssn_loader.py model --info    # Информация о модели
python ssn_loader.py model --test    # Тестирование модели
python ssn_loader.py model --prompt "текст"  # Тест с промптом

Примеры:
python ssn_loader.py ssn_medium --info
python ssn_loader.py --list
python ssn_loader.py ssn_small --test
""")

if __name__ == "__main__":
main()
