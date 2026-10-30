for each entity in world {
// 1. СБОР ДАННЫХ: Прочитать состояния 8 соседних клеток (быстрая битовая операция)
neighbor_states = read_neighbors(entity.position);

// 2. ПРИМЕНЕНИЕ ПРАВИЛА: Определить действие на основе своего "генома" и соседей
// "Геном" — это тоже просто число, маскируемое для получения решения
action = (entity.rule_genome & neighbor_states) % NUMBER_OF_ACTIONS;

// 3. ИСПОЛНЕНИЕ: Минимальное изменение состояния мира
world[new_position] = apply_action(action, entity);
}
