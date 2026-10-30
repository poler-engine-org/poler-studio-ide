## Создано
{metadata['created_at']}
"""

readme_file = model_dir / 'README.md'
with open(readme_file, 'w', encoding='utf-8') as f:
f.write(readme_content)

def list_models(self) -> List[str]:
"""Список доступных моделей"""
models = []
for item in self.models_dir.iterdir():
if item.is_dir():
config_file = item / 'config.json'
if config_file.exists():
with open(config_file, 'r') as f:
config = json.load(f)
models.append({
'name': item.name,
'size': config.get('model_size', 'unknown'),
'created': config.get('created_at', 'unknown'),
'dimensions': config.get('input_dim', 'unknown')
})
return models

def delete_model(self, model_name: str, confirm: bool = True) -> bool:
"""Удаление модели"""
model_dir = self.models_dir / model_name

if not model_dir.exists():
print(f"❌ Модель '{model_name}' не найдена")
return False

if confirm:
print(f"⚠️  Вы собираетесь удалить модель: {model_name}")
print(f"📁 Директория: {model_dir}")
response = input("Продолжить? (y/N): ")
if response.lower() != 'y':
print("Отмена")
return False

import shutil
shutil.rmtree(model_dir)
print(f"✅ Модель '{model_name}' удалена")
return True

def get_model_info(self, model_name: str) -> Dict[str, Any]:
"""Получение информации о модели"""
model_dir = self.models_dir / model_name

if not model_dir.exists():
raise FileNotFoundError(f"Модель '{model_name}' не найдена")

info = {
'path': str(model_dir),
'files': {},
'config': None,
'metadata': None
}

# Загрузка конфигурации
config_file = model_dir / 'config.json'
if config_file.exists():
with open(config_file, 'r') as f:
info['config'] = json.load(f)

# Загрузка метаданных
meta_file = model_dir / 'metadata.json'
if meta_file.exists():
with open(meta_file, 'r') as f:
info['metadata'] = json.load(f)

# Список файлов
for file_path in model_dir.iterdir():
if file_path.is_file():
size = file_path.stat().st_size
info['files'][file_path.name] = {
'size': self._format_bytes(size),
'path': str(file_path)
}

return info

def _format_bytes(self, size: int) -> str:
"""Форматирование размера в байтах"""
for unit in ['B', 'KB', 'MB', 'GB']:
if size < 1024.0:
return f"{size:.1f} {unit}"
size /= 1024.0
return f"{size:.1f} TB"

# ========== ФУНКЦИИ ЗАГРУЗКИ ==========

def load_ssn_model(model_name: str, models_dir: str = 'models'):
"""
Загрузка модели SSN

Args:
model_name: имя модели
models_dir: директория с моделями

Returns:
Dict с загруженными компонентами
"""
model_dir = Path(models_dir) / model_name

if not model_dir.exists():
raise FileNotFoundError(f"Модель '{model_name}' не найдена в {models_dir}")

print(f"📦 Загрузка модели: {model_name}")
print(f"📁 Директория: {model_dir}")

# Загрузка конфигурации
config_file = model_dir / 'config.json'
with open(config_file, 'r') as f:
config = json.load(f)

# Загрузка CSE
print("📖 Загрузка CSE энкодера...")
cse_file = model_dir / 'cse_encoder.pkl'
with open(cse_file, 'rb') as f:
cse_encoder = pickle.load(f)

# Загрузка SSN
print("🧠 Загрузка синаптической сети...")
ssn_file = model_dir / 'ssn_core.pkl'
with open(ssn_file, 'rb') as f:
ssn_core = pickle.load(f)

# Создание интерфейса
print("🔗 Создание интерфейса CSEForSinusoidalNetwork...")
from cse_encoder import CSEForSinusoidalNetwork
cse_interface = CSEForSinusoidalNetwork()
cse_interface.encoder = cse_encoder

print("✅ Модель успешно загружена")

return {
'config': config,
'cse': cse_encoder,
'cse_interface': cse_interface,
'ssn': ssn_core,
'model_dir': str(model_dir)
}

def load_streamflow_sampler(model_name: str, models_dir: str = 'models'):
"""
Загрузка модели с StreamFlow Sampler

Args:
model_name: имя модели
models_dir: директория с моделями

Returns:
StreamFlowSampler готовый к использованию
"""
print(f"🌊 Загрузка StreamFlow Sampler для модели: {model_name}")

# Загрузка компонентов
components = load_ssn_model(model_name, models_dir)

# Импорт и создание сэмплера
from sample import StreamFlowSampler
sampler = StreamFlowSampler(
ssn_core=components['ssn'],
cse_encoder=components['cse_interface'],
flow_dim=components['config']['input_dim']
)

# Настройка параметров по умолчанию
from sample import FlowParameters
sampler.flow_params = FlowParameters(
creativity=0.5,
coherence=0.6,
temperature=0.3
)

print("✅ StreamFlow Sampler готов к работе")

return {
'sampler': sampler,
'components': components
}

# ========== КОМАНДНАЯ СТРОКА ==========

def main():
"""Основная функция для командной строки"""
import argparse

parser = argparse.ArgumentParser(description='SSN Model Initializer')
subparsers = parser.add_subparsers(dest='command', help='Команды')

# Создание модели
create_parser = subparsers.add_parser('create', help='Создание новой модели')
create_parser.add_argument('--name', type=str, default='ssn_medium',
help='Имя модели')
create_parser.add_argument('--size', type=str, default='medium',
choices=['tiny', 'small', 'medium', 'large'],
help='Размер модели')
create_parser.add_argument('--force', action='store_true',
help='Перезаписать существующую модель')

# Список моделей
list_parser = subparsers.add_parser('list', help='Список моделей')

# Информация о модели
info_parser = subparsers.add_parser('info', help='Информация о модели')
info_parser.add_argument('name', type=str, help='Имя модели')

# Удаление модели
delete_parser = subparsers.add_parser('delete', help='Удаление модели')
delete_parser.add_argument('name', type=str, help='Имя модели')
delete_parser.add_argument('--force', action='store_true',
help='Не спрашивать подтверждение')

# Загрузка модели
load_parser = subparsers.add_parser('load', help='Загрузка модели')
load_parser.add_argument('name', type=str, help='Имя модели')
load_parser.add_argument('--sampler', action='store_true',
help='Загрузить с StreamFlow Sampler')

args = parser.parse_args()

# Инициализатор
initializer = SSNInitializer()

if args.command == 'create':
result = initializer.create_model(
model_name=args.name,
model_size=args.size,
force_create=args.force
)

if result:
print(f"\n🎉 Модель создана успешно!")
print(f"📁 Директория: {result['model_dir']}")

elif args.command == 'list':
models = initializer.list_models()

if not models:
print("📭 Модели не найдены")
return

print(f"📚 Найдено моделей: {len(models)}")
print("=" * 80)

for model in models:
print(f"🔸 {model['name']}")
print(f"   Размер: {model['size']}")
print(f"   Размерность: {model['dimensions']}D")
print(f"   Создана: {model['created']}")
print()

elif args.command == 'info':
try:
info = initializer.get_model_info(args.name)

print(f"📊 Информация о модели: {args.name}")
print("=" * 60)

if info['metadata']:
print("📄 Метаданные:")
for key, value in info['metadata'].items():
print(f"  {key}: {value}")
print()

if info['config']:
print("⚙️ Конфигурация:")
for key, value in info['config'].items():
print(f"  {key}: {value}")
print()

if info['files']:
print("📁 Файлы:")
for filename, fileinfo in info['files'].items():
print(f"  {filename} ({fileinfo['size']})")

except FileNotFoundError as e:
print(f"❌ {e}")

elif args.command == 'delete':
success = initializer.delete_model(args.name, confirm=not args.force)

elif args.command == 'load':
try:
if args.sampler:
result = load_streamflow_sampler(args.name)
print(f"\n✅ StreamFlow Sampler загружен")
print(f"🧠 Сеть: {result['components']['config']['input_dim']}D")
print(f"📊 Параметры: творчество={result['sampler'].flow_params.creativity}")
else:
result = load_ssn_model(args.name)
print(f"\n✅ Модель загружена")
print(f"🧠 Сеть: {result['config']['input_dim']}D")
print(f"📖 CSE: {len(result['cse'].axes)} осей")

except Exception as e:
print(f"❌ Ошибка загрузки: {e}")

else:
print("""
🧬 SSN Model Initializer v1.0
=============================
Команды:
create  - Создание новой модели
list    - Список моделей
info    - Информация о модели
delete  - Удаление модели
load    - Загрузка модели

Примеры:
python initialize_ssn.py create --name my_model --size small
python initialize_ssn.py list
python initialize_ssn.py info ssn_medium
""")

if __name__ == "__main__":
main()
