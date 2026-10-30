class InformationFlow:
"""
Поток информации через вашу формулу:
℘ → O → L → ε → R[n] → ℘′ → ...
"""

def __init__(self, n_levels: int = 3):
# Инициализируем фундаментальный 0
self.void = True  # ℘₀ - первичное восприятие отсутствия

# Уровни резонанса
self.R_levels = [[] for _ in range(n_levels)]

# История состояний
self.history = []

# Энергетический баланс
self.energy_budget = 100.0

def perceive(self, signal: Optional[float] = None) -> bool:
"""
℘ (Перцепция): восприятие сигнала относительно вакуума

Правило: сигнал существует ТОЛЬКО если есть фон (0)
"""
if not self.void:
return False  # Невозможно воспринять без фона

if signal is None:
# Восприятие отсутствия (0)
self.current_perception = 0.0
self.perception_type = "absence"
else:
# Восприятие присутствия (1 относительно 0)
self.current_perception = signal
self.perception_type = "presence"

# Сохраняем в историю
self.history.append({
'stage': '℘',
'value': self.current_perception,
'type': self.perception_type,
'relative_to': 'void' if self.void else 'ERROR'
})

return True

def form_image(self, archetypes: Dict[str, float]) -> np.ndarray:
"""
O (Образ): синтез восприятия с памятью и архетипами
"""
if not hasattr(self, 'current_perception'):
raise ValueError("Сначала выполните восприятие (℘)")

# Образ строится как интерференция сигнала с архетипами
image = np.zeros(len(archetypes))

for i, (name, strength) in enumerate(archetypes.items()):
if self.perception_type == "absence":
# Отсутствие → ослабление всех архетипов
image[i] = strength * 0.1
else:
# Присутствие → резонанс с подходящими архетипами
resonance = self.current_perception * strength
image[i] = resonance

self.current_image = image

self.history.append({
'stage': 'O',
'image': image.copy(),
'dominant_archetype': list(archetypes.keys())[np.argmax(image)]
})

return image

def apply_logic(self, rules: Dict[str, callable]) -> Dict:
"""
L (Логика): структура связей и последствий

Особенность: логика работает ПО-РАЗНОМУ для 0 и 1
"""
logic_results = {}

for rule_name, rule_func in rules.items():
if self.perception_type == "absence":
# Логика отсутствия: что МОГЛО БЫ быть
logic_results[rule_name] = {
'result': rule_func(0.0),
'type': 'potential',
'certainty': 0.3  # Низкая уверенность для отсутствия
}
else:
# Логика присутствия: что ЕСТЬ
logic_results[rule_name] = {
'result': rule_func(self.current_perception),
'type': 'actual',
'certainty': 0.8  # Высокая уверенность для присутствия
}

self.current_logic = logic_results

self.history.append({
'stage': 'L',
'logic': logic_results,
'asymmetry': '0≠1'  # Ключевое: логика асимметрична!
})

return logic_results

def compute_energy(self) -> float:
"""
ε (Энергия): значимость мысли

Формула: (∂Значение / ∂Важность) × Интенсивность

Ключ: энергия ВСЕГДА положительна, но для 0 она минимальна
"""
if self.perception_type == "absence":
# Энергия отсутствия (фоновая)
base_energy = 0.001  # Почти 0, но не совсем

# Важность: как сильно мы замечаем отсутствие?
importance_gradient = 0.1  # Мало кто замечает отсутствие

intensity = 1.0  # Базовая интенсивность

else:
# Энергия присутствия
base_energy = abs(self.current_perception)

# Производная значения по важности
# Чем важнее сигнал, тем быстрее растет его значение
importance_gradient = np.tanh(base_energy * 10)

# Интенсивность = нормализованное значение
intensity = base_energy / (base_energy + 1)

# Итоговая энергия
energy = importance_gradient * intensity

# Сохраняем с учетом асимметрии
self.current_energy = {
'value': energy,
'type': 'absence_energy' if self.perception_type == "absence" else 'presence_energy',
'gradient': importance_gradient,
'intensity': intensity
}

self.history.append({
'stage': 'ε',
'energy': self.current_energy,
'asymmetric_formula': '∂V/∂I × Intensity'
})

return energy

def resonate(self, n: int = 3) -> List[Dict]:
"""
R[n] (Резонанс): многоуровневая ответная реакция

Особенность: резонанс ВОЗМОЖЕН только если есть 0 как "пространство"
"""
if not self.void:
return []  # Без вакуума нет пространства для резонанса

resonances = []

for level in range(n):
# Глубина резонанса зависит от уровня
depth = 2 ** level

if self.perception_type == "absence":
# Резонанс отсутствия: эхо в пустоте
resonance = {
'level': level,
'amplitude': 0.01 / depth,  # Затухающее эхо
'frequency': 1.0 / (level + 1),
'type': 'echo_in_void'
}
else:
# Резонанс присутствия: волны в среде
amplitude = self.current_perception / depth
resonance = {
'level': level,
'amplitude': amplitude,
'frequency': self.current_perception * (level + 1),
'type': 'wave_in_medium',
'medium': 'void'  # Среда ВСЕГДА вакуум!
}

resonances.append(resonance)

# Сохраняем в соответствующий уровень
self.R_levels[level].append(resonance)

self.current_resonance = resonances

self.history.append({
'stage': f'R[{n}]',
'resonances': resonances,
'requires_void': True  # Ключевое условие!
})

return resonances

def update_perception(self, feedback_from_R: List[Dict]) -> float:
"""
℘′ → ... : Обновленное восприятие после резонанса

Это НОВОЕ восприятие, но все равно относительно изначального 0
"""
if not self.void:
return 0.0

# Суммируем резонансы
total_feedback = sum([r['amplitude'] for r in feedback_from_R])

# Новое восприятие = старое + резонанс
# Но ВСЕГДА относительно изначального вакуума
if self.perception_type == "absence":
# Отсутствие + резонанс = возможное появление
new_perception = total_feedback * 0.1
else:
# Присутствие + резонанс = усиление/ослабление
new_perception = self.current_perception + total_feedback

# Обновляем, но сохраняем связь с вакуумом
self.perception_type = "presence" if new_perception > 0.01 else "absence"
self.current_perception = new_perception

# Цикл продолжается...
self.history.append({
'stage': '℘′',
'new_perception': new_perception,
'still_relative_to': 'void',
'cycle_continues': True
})

return new_perception

def run_full_cycle(self,
signal: Optional[float] = None,
archetypes: Optional[Dict] = None,
rules: Optional[Dict] = None,
n_resonance: int = 3) -> Dict:
"""
Полный цикл: ℘ → O → L → ε → R[n] → ℘′
"""
# Начинаем ВСЕГДА с вакуума
print("🌌 Начало цикла из вакуума (0)...")

# ℘
self.perceive(signal)
print(f"  ℘: {self.perception_type} = {self.current_perception}")

# O
archetypes = archetypes or {'archetype_0': 1.0}
self.form_image(archetypes)
print(f"  O: образ из {len(archetypes)} архетипов")

# L
rules = rules or {
'existence': lambda x: x > 0,
'intensity': lambda x: abs(x)
}
self.apply_logic(rules)
print(f"  L: применено {len(rules)} правил (асимметрично)")

# ε
energy = self.compute_energy()
print(f"  ε: энергия = {energy:.4f} ({self.current_energy['type']})")

# R[n]
resonances = self.resonate(n_resonance)
print(f"  R[{n_resonance}]: {len(resonances)} уровней резонанса")

# ℘′
new_perception = self.update_perception(resonances)
print(f"  ℘′: новое восприятие = {new_perception:.4f}")
print(f"  Цикл продолжается...")

return {
'final_perception': new_perception,
'energy': energy,
'resonance_levels': len(resonances),
'history_length': len(self.history),
'void_persists': self.void  # Ключевой инвариант!
}

# Тест системы
print("\n" + "="*60)
print("🧠 Тест асимметричного информационного потока")
print("="*60)

flow = InformationFlow(n_levels=3)

# Тест 1: Отсутствие сигнала (чистый 0)
print("\n📭 Тест 1: Чистое отсутствие (0 без 1)")
result1 = flow.run_full_cycle(signal=None)
print(f"Результат: {result1}")

# Тест 2: Слабое присутствие (1 относительно 0)
print("\n📡 Тест 2: Слабое присутствие (1 из 0)")
result2 = flow.run_full_cycle(signal=0.3)
print(f"Результат: {result2}")

# Тест 3: Сильное присутствие
print("\n💥 Тест 3: Сильное присутствие")
result3 = flow.run_full_cycle(signal=0.9)
print(f"Результат: {result3}")

print("\n" + "="*60)
print("📊 Итоговая статистика:")
print(f"Всего шагов в истории: {len(flow.history)}")
print(f"Вакуум сохранился: {flow.void}")
print(f"Резонансных уровней: {len(flow.R_levels)}")
print("="*60)
