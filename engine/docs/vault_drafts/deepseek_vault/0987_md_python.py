"""
Главный скрипт Sinusoidal Synaptic Network
"""

import sys
import argparse
from pathlib import Path

def main():
parser = argparse.ArgumentParser(description='Sinusoidal Synaptic Network v1.0')

subparsers = parser.add_subparsers(dest='command', help='Команды')

# Интерфейс GPT-2 совместимости
gpt2_parser = subparsers.add_parser('gpt2', help='GPT-2 совместимый интерфейс')
gpt2_parser.add_argument('--mode', choices=['interactive', 'generate'], default='interactive')

# Потоковый интерфейс
stream_parser = subparsers.add_parser('stream', help='Потоковая обработка')
stream_parser.add_argument('--input', type=str, help='Входной текст или файл')

# Обучение
train_parser = subparsers.add_parser('train', help='Обучение сети')
train_parser.add_argument('--dataset', type=str, required=True)

args = parser.parse_args()

if args.command == 'gpt2':
from interactive_conditional_samples import fire_interact
fire_interact()
elif args.command == 'stream':
print("Запуск потоковой обработки...")
elif args.command == 'train':
print("Запуск обучения...")
else:
print("""
🧬 Sinusoidal Synaptic Network v1.0
===================================
Команды:
gpt2    - GPT-2 совместимый интерфейс
stream  - Потоковая обработка
train   - Обучение на датасете
""")

if __name__ == "__main__":
main()
