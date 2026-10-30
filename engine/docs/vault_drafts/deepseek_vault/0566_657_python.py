state = initial_hidden
program = []

for step in range(max_program_length):
# state содержит информацию о том, что происходит с данными
    op_token = program_generator(state)  # классификатор на N операций
program.append(op_token)

# ИСПОЛНЯЕМ эту операцию прямо сейчас на реальных данных
# (differentiable execution)
state = execute_operation(state, op_token)

# Потери: насколько хорошо state помогает решить задачу агента
loss = task_loss(state, target)

# Градиент идёт в program_generator
# Он учится писать программу, которая минимизирует loss
