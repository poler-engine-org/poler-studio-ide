extends Camera

export var target_path: NodePath
export var distance = 10.0
export var height = 5.0
export var smooth_speed = 5.0

var target: Spatial

func _ready():
target = get_node(target_path)

func _process(delta):
if target:
# Желаемая позиция камеры: сзади и сверху от цели
var desired_position = target.global_transform.translated(Vector3(0, height, distance)).origin
# Плавное перемещение камеры к желаемой позиции
global_transform.origin = global_transform.origin.linear_interpolate(desired_position, smooth_speed * delta)
# Камера всегда смотрит на цель
look_at(target.global_transform.origin, Vector3.UP)
