# Поднимаем всю машину (кроме пола)
bpy.ops.object.select_all(action='DESELECT')
for obj in bpy.data.objects:
if obj.type == 'MESH' and obj.name != "Studio_Floor" and "plane" not in obj.name.lower():
obj.select_set(True)
# Поднимаем на высоту, равную радиусу колеса (0.35) + небольшой зазор (0.1)
bpy.ops.transform.translate(value=(0, 0, 0.45))
