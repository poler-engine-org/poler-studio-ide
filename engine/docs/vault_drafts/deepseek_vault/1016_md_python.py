"""
Асимметричная информационная система: 0 ↮ 1
"""
import numpy as np
from typing import Dict, List, Optional, Tuple

class AsymmetricBit:
"""
Асимметричный бит: 0 может существовать без 1, но 1 требует 0 как референс
"""

def __init__(self):
self.void = True          # Фундаментальный ноль (вакуум)
self.presence = False     # Присутствие (возникает из вакуума)
self.reference = None     # Референс для 1 (всегда 0)

def emerge(self, probability: float) -> bool:
"""
Возникновение 1 из 0

1. Сначала должен существовать 0 (void=True)
2. Только потом может появиться 1
3. 1 всегда определяется относительно 0
"""
if not self.void:
raise ValueError("❌ Невозможно: 1 не может существовать без 0")

# Вероятность возникновения из вакуума
if np.random.random() < probability:
self.presence = True
self.reference = self.void  # Референс: сам вакуум
return True
return False

def collapse(self) -> bool:
"""Коллапс 1 обратно в 0 (но 0 остается)"""
if self.presence:
self.presence = False
# 0 продолжает существовать!
self.void = True
return True
return False

def get_state(self) -> str:
"""Состояние с учетом асимметрии"""
if self.void and not self.presence:
return "0 (самосущий)"
elif self.void and self.presence:
return f"1 (относительно {self.reference})"
else:
return "IMPOSSIBLE_STATE"

# Тест асимметрии
print("🧪 Тест асимметричной логики:")
bit = AsymmetricBit()
print(f"Начальное состояние: {bit.get_state()}")  # 0 (самосущий)

bit.emerge(0.5)
print(f"После emergence: {bit.get_state()}")     # 1 (относительно 0)

bit.collapse()
print(f"После collapse: {bit.get_state()}")      # 0 (самосущий) - ВСЕГДА остается
