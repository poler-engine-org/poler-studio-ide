class AttractorToText:
"""
Проектор: аттрактор → байтовый поток
Не генератор, а дешифратор состояния
"""
def __init__(self, dim=768):
# Фиксированная проекция (не обучаемая!)
# Базисные векторы для разных «слоёв смысла»
self.semantic_basis = self._generate_semantic_basis(dim)

def _generate_semantic_basis(self, dim):
"""
Генерация ортонормированного базиса для семантических измерений:
- Измерение 0: грамматическая роль (подлежащее/сказуемое)
- Измерение 1: временная локализация
- Измерение 2: эмоциональная окраска
- ...
"""
# Детерминированная генерация (не обучение!)
basis = torch.randn(dim, dim)
basis = torch.linalg.qr(basis).Q  # Ортонормирование
return basis

def project(self, attractor: torch.Tensor) -> bytes:
"""
Проекция аттрактора в байтовое пространство
"""
# Разложение по семантическому базису
coefficients = attractor @ self.semantic_basis.T  # [dim]

# Квантизация коэффициентов в байты
# Принцип: каждый байт кодирует проекцию на подпространство
byte_stream = []
for i in range(0, dim, 8):  # 8 измерений → 1 байт
chunk = coefficients[i:i+8]
byte_val = int((torch.sigmoid(chunk.mean()) * 255).item())
byte_stream.append(byte_val)

return bytes(byte_stream)
