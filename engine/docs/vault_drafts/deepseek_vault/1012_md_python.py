class ThinkingPrimitives:
"""Примитивы мышления — базовые операторы"""

primitives = {
# Базовые передачи
'TRANSMIT': lambda s, w, x: s + w * x,

# Ассоциативные
'BIND': lambda A, B, g: A * g + B * (1 - g),  # Связывание
'MERGE': lambda A, B: (A + B) / 2,            # Слияние
'COMPARE': lambda A, B: np.abs(A - B),        # Сравнение

# Логические
'AND': lambda A, B: np.minimum(A, B),
'OR': lambda A, B: np.maximum(A, B),
'NOT': lambda A: 1 - A,
'IMPLIES': lambda A, B: np.maximum(1 - A, B),

# Структурные
'GROUP': lambda items, key: self.group_by(items, key),
'SEQUENCE': lambda items: self.temporal_sequence(items),
'HIERARCHY': lambda items: self.build_hierarchy(items),

# Абстрактные
'ABSTRACT': lambda concrete: self.extract_pattern(concrete),
'GENERALIZE': lambda specific, examples: self.find_commonality(specific, examples),
'ANALOGY': lambda source, target: self.map_analogy(source, target),
}

def compose(self, primitive_chain):
"""Композиция примитивов в сложные мысли"""
current_state = self.context_state

for primitive in primitive_chain:
op = self.primitives[primitive['op']]
args = primitive.get('args', [])
current_state = op(current_state, *args)

return current_state
