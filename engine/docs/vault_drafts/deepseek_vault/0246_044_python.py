import bpy
import bmesh
import math
import os

def create_complex_racecar_chassis():
# Clear scene
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)

# -------- Настройки посадки --------
    ride_height = 0.35   # ось колеса над землёй
wheel_radius = 0.35

# 1. Колесо (без изменений, но координаты Z теперь передаются)
def create_wheel(name, loc, is_left=True):
mesh = bpy.data.meshes.new(name + "_mesh")
obj = bpy.data.objects.new(name, mesh)
bpy.context.collection.objects.link(obj)
obj.location = loc

bm = bmesh.new()
tire_radius = wheel_radius
tire_width = 0.28
segments = 32

verts_outer = []
verts_inner = []
for i in range(segments):
angle = (i / segments) * 2.0 * math.pi
y = math.sin(angle) * tire_radius
z = math.cos(angle) * tire_radius
sign = -1.0 if is_left else 1.0
v_out = bm.verts.new((0, y, z))
v_in = bm.verts.new((sign * tire_width, y, z))
verts_outer.append(v_out)
verts_inner.append(v_in)
bm.verts.ensure_lookup_table()
for i in range(segments):
next_i = (i + 1) % segments
bm.faces.new((verts_outer[i], verts_inner[i], verts_inner[next_i], verts_outer[next_i]))

# Диск и ротор (без изменений)
hub_center = bm.verts.new(((-0.05 if is_left else 0.05) * tire_width, 0, 0))
rotor_rad = 0.22
for s in range(5):
angle1 = (s / 5.0) * 2.0 * math.pi
angle2 = ((s + 0.35) / 5.0) * 2.0 * math.pi
p1_y, p1_z = math.sin(angle1) * (tire_radius * 0.75), math.cos(angle1) * (tire_radius * 0.75)
p2_y, p2_z = math.sin(angle2) * (tire_radius * 0.75), math.cos(angle2) * (tire_radius * 0.75)
v_rim1 = bm.verts.new(((-0.08 if is_left else 0.08) * tire_width, p1_y, p1_z))
v_rim2 = bm.verts.new(((-0.08 if is_left else 0.08) * tire_width, p2_y, p2_z))
bm.faces.new((hub_center, v_rim1, v_rim2))

bm.to_mesh(mesh)
bm.free()
return obj

# Создаём колёса с правильной высотой
w_fl = create_wheel("Wheel_FL", (-1.0, 1.45, ride_height), True)
w_fr = create_wheel("Wheel_FR", (1.0, 1.45, ride_height), False)
w_rl = create_wheel("Wheel_RL", (-1.05, -1.45, ride_height), True)
w_rr = create_wheel("Wheel_RR", (1.05, -1.45, ride_height), False)

# 2. Кузов – все Z координаты увеличены на ride_height + небольшой зазор
body_mesh = bpy.data.meshes.new("Supercar_Body")
body_obj = bpy.data.objects.new("Supercar_Chassis", body_mesh)
bpy.context.collection.objects.link(body_obj)

bm_body = bmesh.new()
# Немного поднимем кузов над колёсами (добавляем 0.05 к ride_height)
body_z_offset = ride_height + 0.05

verts_data = [
(0.0, 2.5, 0.08 + body_z_offset),
(-0.85, 2.35, 0.08 + body_z_offset),
(-0.95, 1.8, 0.12 + body_z_offset),
(-0.98, 1.1, 0.15 + body_z_offset),
(-0.92, -0.8, 0.12 + body_z_offset),
(-1.02, -1.1, 0.15 + body_z_offset),
(-1.02, -1.8, 0.18 + body_z_offset),
(-0.88, -2.5, 0.22 + body_z_offset),
(0.0, -2.55, 0.25 + body_z_offset),
(0.0, 2.3, 0.52 + body_z_offset),
(-0.75, 2.1, 0.55 + body_z_offset),
(-0.80, 0.6, 0.68 + body_z_offset),
(0.0, 0.65, 0.65 + body_z_offset),
(-0.85, -1.3, 0.78 + body_z_offset),
(-0.80, -2.35, 0.72 + body_z_offset),
(0.0, -2.4, 0.70 + body_z_offset),
(0.0, 0.35, 1.05 + body_z_offset),
(-0.52, 0.35, 1.02 + body_z_offset),
(-0.48, -0.95, 0.98 + body_z_offset),
(0.0, -0.95, 1.00 + body_z_offset),
]

bm_verts = [bm_body.verts.new(p) for p in verts_data]
bm_body.verts.ensure_lookup_table()

faces_indices = [
(0, 1, 10, 9),
(9, 10, 11, 12),
(1, 2, 10),
(2, 3, 11),
(3, 4, 11),
(4, 5, 13, 11),
(5, 6, 13),
(6, 7, 14, 13),
(7, 8, 15, 14),
(13, 14, 15),
(12, 11, 17, 16),
(16, 17, 18, 19),
(19, 18, 13, 15),
(11, 13, 18, 17),
]
for f in faces_indices:
bm_body.faces.new([bm_verts[idx] for idx in f])

bm_body.to_mesh(body_mesh)
bm_body.free()

# 3. Модификаторы
bpy.context.view_layer.objects.active = body_obj
mirror_mod = body_obj.modifiers.new(name="Mirror", type='MIRROR')
mirror_mod.use_clip = True
subdiv_mod = body_obj.modifiers.new(name="Subdivision", type='SUBSURF')
subdiv_mod.levels = 1
subdiv_mod.render_levels = 2

# 4. Крыло – тоже поднимаем
wing_mesh = bpy.data.meshes.new("GT_Wing_Mesh")
wing_obj = bpy.data.objects.new("GT_Rear_Wing", wing_mesh)
bpy.context.collection.objects.link(wing_obj)

bm_wing = bmesh.new()
    wing_z = ride_height + 0.10   # чуть выше кузова
w_v1 = bm_wing.verts.new((-0.95, -2.2, 1.15 + wing_z))
w_v2 = bm_wing.verts.new((0.95, -2.2, 1.15 + wing_z))
w_v3 = bm_wing.verts.new((0.95, -2.45, 1.22 + wing_z))
w_v4 = bm_wing.verts.new((-0.95, -2.45, 1.22 + wing_z))
bm_wing.faces.new((w_v1, w_v2, w_v3, w_v4))

for mx in [-0.45, 0.45]:
p1 = bm_wing.verts.new((mx - 0.03, -2.15, 0.72 + wing_z))
p2 = bm_wing.verts.new((mx + 0.03, -2.15, 0.72 + wing_z))
p3 = bm_wing.verts.new((mx + 0.03, -2.35, 1.16 + wing_z))
p4 = bm_wing.verts.new((mx - 0.03, -2.35, 1.16 + wing_z))
bm_wing.faces.new((p1, p2, p3, p4))

bm_wing.to_mesh(wing_mesh)
bm_wing.free()

# 5. Материалы (без изменений)
def create_car_paint_mat(name, color, roughness=0.1, metallic=0.9):
mat = bpy.data.materials.new(name=name)
mat.use_nodes = True
nodes = mat.node_tree.nodes
bsdf = nodes.get("Principled BSDF")
if bsdf:
bsdf.inputs['Base Color'].default_value = color
bsdf.inputs['Roughness'].default_value = roughness
bsdf.inputs['Metallic'].default_value = metallic
return mat

red_paint = create_car_paint_mat("Ferrari_Red_Metallic", (0.85, 0.02, 0.02, 1.0), roughness=0.08, metallic=0.95)
carbon_black = create_car_paint_mat("Carbon_Fiber", (0.05, 0.05, 0.05, 1.0), roughness=0.3, metallic=0.2)
rubber = create_car_paint_mat("Tire_Rubber", (0.02, 0.02, 0.02, 1.0), roughness=0.7, metallic=0.0)

body_obj.data.materials.append(red_paint)
wing_obj.data.materials.append(carbon_black)
w_fl.data.materials.append(rubber)
w_fr.data.materials.append(rubber)
w_rl.data.materials.append(rubber)
w_rr.data.materials.append(rubber)

# 6. Пол и освещение (без изменений)
bpy.ops.mesh.primitive_plane_add(size=30, location=(0, 0, 0))
ground = bpy.context.object
ground.data.materials.append(create_car_paint_mat("Studio_Floor", (0.12, 0.12, 0.14, 1.0), roughness=0.2, metallic=0.1))

bpy.ops.object.camera_add(location=(4.8, -5.5, 2.2), rotation=(1.25, 0.0, 0.72))
bpy.context.scene.camera = bpy.context.object

bpy.ops.object.light_add(type='SUN', location=(6, -6, 12))
sun = bpy.context.object
sun.data.energy = 4.0
bpy.ops.object.light_add(type='POINT', location=(-4, 4, 3))
rim = bpy.context.object
rim.data.energy = 800.0
rim.data.color = (0.3, 0.6, 1.0)

bpy.context.scene.render.engine = 'CYCLES'
bpy.context.scene.cycles.samples = 32
bpy.context.scene.render.resolution_x = 1280
bpy.context.scene.render.resolution_y = 720
bpy.context.scene.render.filepath = os.path.join(os.getcwd(), "complex_car_render.png")
bpy.ops.render.render(write_still=True)
    print("[P3-Blender] Supercar поднят, колёса снаружи, реализм улучшен!")

if __name__ == "__main__":
create_complex_racecar_chassis()
