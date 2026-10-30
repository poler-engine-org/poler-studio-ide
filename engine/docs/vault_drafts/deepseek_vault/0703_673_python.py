# trainer.py
import random
import time

patterns = [
"def hello():",
"    print('Hello')",
"for i in range(10):",
"    print(i)"
]

while True:
pattern = random.choice(patterns)
    print("Повторите:", pattern)
time.sleep(2)
# Здесь система сравнивает, что вы напечатали
