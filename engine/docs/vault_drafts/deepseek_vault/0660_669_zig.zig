extends Camera

export var target_path: NodePath
export var offset = Vector3(0, 5, 10)  # относительно машины
export var smooth_speed = 5.0

var target: Spatial

func _ready():
target = get_node(target_path)

func _process(delta):
if target:
# Желаемая позиция в мировых координатах, учитывая текущий поворот машины
var desired_position = target.global_transform * offset
# Плавное перемещение
global_transform.origin = global_transform.origin.linear_interpolate(desired_position, smooth_speed * delta)
# Поворачиваем камеру, чтобы смотреть на машину
look_at(target.global_transform.origin, Vector3.UP)
