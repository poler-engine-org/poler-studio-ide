class ResonanceMemory:
def __init__(self, capacity=1000, decay_factor=0.99):
        self.memory = []  # Кадры: (вектор, значимость, метка времени)
self.capacity = capacity
self.decay = decay_factor

def add_frame(self, vector, energy):
# Энергия ε вычисляется как дистанция от текущего к прошлому
significant = energy > 0.7

if significant:
frame = {
'vector': vector,
'energy': energy,
'timestamp': time.time(),
'weight': 1.0
}
self.memory.append(frame)

# Автоматическое забывание
self.forget_low_energy()

def forget_low_energy(self):
# Уменьшаем вес со временем
for frame in self.memory:
frame['weight'] *= self.decay

# Удаляем малозначимые
self.memory = [f for f in self.memory
if f['weight'] > 0.1][-self.capacity:]

def get_resonance(self, current_vector):
if not self.memory:
return None

# Находим наиболее резонирующий кадр
resonances = []
        for frame in self.memory[-100:]:  # Последние 100 для скорости
distance = np.linalg.norm(current_vector - frame['vector'])
resonance = frame['weight'] / (1 + distance)
resonances.append((resonance, frame))

return max(resonances, key=lambda x: x[0]) if resonances else None
Матрица взаимодействий W (обучаемая в реальном времени)
python
