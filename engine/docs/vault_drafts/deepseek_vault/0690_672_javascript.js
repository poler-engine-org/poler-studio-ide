function text_to_attractor(text::String, ops::ConstraintOperators)
# Обратная задача: найти состояние, проекция которого ≈ text
# Решаем оптимизацию: min ||project(state) - text_bytes||²

# Начальное приближение через анализ текста
initial_guess = analyze_text_structure(text)

# Ищем состояние, удовлетворяющее ограничениям
attractor = find_state_satisfying_constraints(
initial_guess,
ops,
additional_constraint = state ->
norm(project_attractor_to_bytes(state, length(text)) - text_to_bytes(text))
)

return attractor
end
