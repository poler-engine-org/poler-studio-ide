# Инициализация системы БЕЗ весов
system = ConstraintCognitiveSystem(
dim=768,
constraints=[
grammatical_constraint,   # Грамматика как функция ℝ^d → ℝ
logical_constraint,       # Логика как функция ℝ^d → ℝ
ontological_constraint    # Онтология как функция ℝ^d → ℝ
]
)

# Ввод: намерение (не текст!)
intention = system.encode_intention("описать восход солнца над горами")

# Эволюция до аттрактора
attractor = system.evolve_to_attractor(intention)

# Проекция в текст
text_bytes = system.project_attractor(attractor)
text = text_bytes.decode('utf-8', errors='ignore')

print(text)
# → "Солнце медленно поднималось над зубчатым гребнем гор, окрашивая небо в багрянец..."
