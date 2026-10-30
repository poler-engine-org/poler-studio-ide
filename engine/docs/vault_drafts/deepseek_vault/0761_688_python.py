# Тест 1: Проверка частоты 0.125
def test_resonance_frequency():
for d in [8, 10, 11, 16, 26, 32, 64]:
p = random_state(d)
f = measure_resonance_frequency(p)
        assert abs(f - 0.125) < 0.001  # Должна сохраняться

# Тест 2: Сохранение/нарушение нормы
def test_norm_conservation():
# Консервативный режим
p = evolve(p0, D=0, J=1)
assert abs(norm(p) - 1) < 1e-10

# Диссипативный режим
p = evolve(p0, D=0.1, J=0)
    assert norm(p) < 1  # Нарушение нормы

# Тест 3: Вероятностный коллапс
def test_probabilistic_collapse():
responses = []
for _ in range(1000):
r = generate_response(T)
responses.append(r)

# Распределение должно быть мультимодальным
entropy = calculate_entropy(responses)
