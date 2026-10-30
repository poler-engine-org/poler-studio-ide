# У твоєму minimal_sim.py додай ці дані в __init__
self.planets = [
{"name": "Геліос", "type": "star", "mass": 0.2, "distance": 0, "period": 0, "angle": 0},
{"name": "Ефір", "type": "brown_dwarf", "mass": 50, "distance": 10, "period": 5000, "angle": 0, "resonance": None},
{"name": "Планета 1", "type": "rocky", "mass": 0.1, "distance": 0.1, "period": 30, "angle": 0},
{"name": "Планета 2", "type": "rocky", "mass": 0.5, "distance": 0.2, "period": 80, "angle": 0},
{"name": "Планета 3", "type": "rocky", "mass": 0.8, "distance": 0.4, "period": 150, "angle": 0},
{"name": "Планета 4", "type": "mini_neptune", "mass": 10, "distance": 0.6, "period": 220, "angle": 0},
{"name": "Планета 5", "type": "gas_giant", "mass": 20, "distance": 0.9, "period": 380, "angle": 0},
{"name": "Планета 6", "type": "gas_giant", "mass": 50, "distance": 1.2, "period": 252.5, "angle": 0, "resonance": "2:1_with_Kronos"}, # Резонанс 2:1 з Кроносом
{"name": "Кронос", "type": "gas_giant", "mass": 953.4, "distance": 1.5, "period": 505, "angle": 0, "resonance": "3:2_with_Planet8"},
{"name": "Кассіопея", "type": "moon", "mass": 1.4, "distance": 0.01, "period": 20, "angle": 0, "parent": "Кронос"},
{"name": "Планета 8", "type": "ice_giant", "mass": 15, "distance": 2.0, "period": 617, "angle": 0, "resonance": "2:1_with_Planet9"}, # 2:1 з Планетою 9
{"name": "Планета 9", "type": "ice_giant", "mass": 12, "distance": 3.0, "period": 1234, "angle": 0},
]

def enforce_resonances(self):
    """Функція, що забезпечує дотримання резонансів між планетами"""
for planet in self.planets:
if planet.get('resonance') == '2:1_with_Kronos' and planet['name'] == 'Планета 6':
# Примусово синхронізуємо період Планети 6 до резонансу 2:1 з Кроносом
kronos = next(p for p in self.planets if p['name'] == 'Кронос')
