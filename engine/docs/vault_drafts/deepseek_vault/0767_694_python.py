class DynamicWeights:
def __init__(self, n_archetypes):
        self.W = np.eye(n_archetypes) * 0.1  # Начинаем с нейтрального
self.learning_rate = 0.01
self.constraints = self.load_constraints()

def apply(self, input_vector):
output = np.dot(self.W, input_vector)

# Применяем ограничения Π_Λ
for i, val in enumerate(output):
archetype_name = self.index_to_name(i)
if archetype_name in self.constraints['forbidden']:
output[i] = 0.0
elif archetype_name in self.constraints['max_values']:
output[i] = min(val, self.constraints['max_values'][archetype_name])

return output

def learn(self, input_vector, correction_vector, feedback):
# Обновляем веса по правилу Хебба
error = correction_vector - np.dot(self.W, input_vector)
adjustment = self.learning_rate * np.outer(error, input_vector)

# Учёт обратной связи
adjustment *= feedback  # feedback от -1.0 до 1.0

self.W += adjustment

# Нормализация для стабильности
self.W = np.clip(self.W, -1.0, 1.0)
3. ПОЛНАЯ СИСТЕМА ВЗАИМОДЕЙСТВИЯ
text
