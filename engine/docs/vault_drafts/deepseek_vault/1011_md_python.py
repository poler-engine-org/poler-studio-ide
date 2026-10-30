class ContextField:
"""Поле состояний с иерархией"""

def __init__(self, dim=2048):
self.layers = {
'perceptual': np.zeros(dim // 8),    # Сенсорный вход
'working': np.zeros(dim // 4),       # Рабочая память
'associative': np.zeros(dim // 2),   # Ассоциативная память
'conceptual': np.zeros(dim),         # Концептуальный уровень
'executive': np.zeros(dim // 16),    # Исполнительный контроль
'metacognitive': np.zeros(dim // 32) # Мета-мышление
}

# Динамические связи между слоями
self.connectivity = DynamicConnectivity()

def propagate(self, signal):
"""Распространение через иерархию"""
# Bottom-up
self.layers['perceptual'] = signal

# Иерархическая обработка
for from_layer, to_layer in [('perceptual', 'working'),
('working', 'associative'),
('associative', 'conceptual'),
('conceptual', 'executive'),
('executive', 'metacognitive')]:

# Гейтированная передача
gate = self.connectivity.gate(from_layer, to_layer)
activation = self.layers[from_layer] * gate

# Нормализация
norm = np.linalg.norm(activation)
if norm > 0:
activation = activation / norm

self.layers[to_layer] = 0.7 * self.layers[to_layer] + 0.3 * activation
