class OperatorPlasticity:
"""Динамическое изменение операторов"""

def adapt_operators(self, state_history, reward):
"""
Онлайн-адаптация операторов на основе опыта

state_history: траектория состояний
reward: сигнал успешности/ошибки
"""

# Анализ эффективности операторов
operator_performance = {}

for op_name, operator in self.operators.items():
# Анализ вклада в достижение цели
contribution = self.analyze_contribution(op_name, state_history)

# Коррекция параметров оператора
if reward > 0:
# Усиление успешных операторов
operator.strengthen(contribution * reward)
else:
# Ослабление неуспешных
operator.weaken(abs(reward) * 0.5)

# Создание новых операторов через композицию
if contribution > 0.8 and reward > 0.7:
self.create_composite_operator([op_name, 'ASSOCIATE'])

def create_composite_operator(self, primitive_chain):
"""Создание нового оператора через композицию"""
# Синтез нового оператора
new_op = CompositeOperator(primitive_chain)

# Тестирование на валидационных данных
performance = self.test_operator(new_op)

if performance > 0.6:  # Порог полезности
self.operators[f"composite_{len(self.operators)}"] = new_op
