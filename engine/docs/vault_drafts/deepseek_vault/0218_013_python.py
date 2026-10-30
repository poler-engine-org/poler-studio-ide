class NaiveBridge:
def vortex_to_poler(vortex_activations):
# Берем 512 самых активных нейронов
top_indices = np.argsort(vortex_activations)[-512:]
return vortex_activations[top_indices]

def poler_to_vortex(poler_vector, vortex_size):
# Распределяем по случайным нейронам
pattern = np.zeros(vortex_size)
for i, value in enumerate(poler_vector):
if abs(value) > 0.1:
neuron_idx = (i * 37) % vortex_size
pattern[neuron_idx] = value
return pattern
День 3-4: Тестовая петля
python
