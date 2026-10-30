import bpy

# Создать куб
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 0))

# Создать сферу
bpy.ops.mesh.primitive_uv_sphere_add(radius=1, location=(3, 0, 0))

# Создать цилиндр
bpy.ops.mesh.primitive_cylinder_add(radius=0.5, depth=2, location=(0, 3, 0))
