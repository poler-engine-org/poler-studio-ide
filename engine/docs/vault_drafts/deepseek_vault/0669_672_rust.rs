# Использование в Python
import synaptics
import numpy as np

# Инициализация Rust движка
engine = synaptics.SynapticEngine(
window_size=256,
embedding_dim=768,
constraint_dim=1024
)

# Потоковая обработка
data_stream = (np.random.randn(1, 768) for _ in range(1000))

window = []
for data in data_stream:
window.append(data)
if len(window) >= 256:
# Веса генерируются в Rust в реальном времени!
result = engine.process_window(window)
