# Lift entire car so that wheels touch ground and chassis is above
bpy.ops.object.select_all(action='DESELECT')
for obj in bpy.data.objects:
if obj.type == 'MESH':
obj.select_set(True)
bpy.ops.transform.translate(value=(0, 0, 0.1))  # поднять на 0.1
