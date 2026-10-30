# sctp_py/api.py
import julia
from julia import SynapticDynamics as sd

class LanguageConstructor:
    """Высокоуровневый интерфейс для конструирования текста"""

def __init__(self, config_path):
self.ops = sd.load_operators(config_path)
self.constraints = sd.load_constraint_system(config_path)

def construct(self, intention: dict, style: str = "neutral") -> str:
        """Конструирование текста из намерения"""
# Преобразуем намерение в начальное состояние
initial_state = self.encode_intention(intention)

# Добавляем стилистические ограничения
style_constraint = self.get_style_constraint(style)
constraints = self.constraints + [style_constraint]

# Находим аттрактор
attractor = sd.solve_attractor(
initial_state,
self.ops,
constraints,
method="adaptive_rk45"
)

# Проекция в текст
return sd.project_to_text(attractor, encoding="utf8")
