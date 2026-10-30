import bpy
import os
import glob

# Укажи путь к своей папке с моделями
model_dir = '/путь/к/твоей/папке/с/моделями/'

# Ищем все .obj файлы
model_files = glob.glob(os.path.join(model_dir, "*.obj"))

for f in model_files:
# Импортируем файл
bpy.ops.wm.obj_import(filepath=f)

# Создаём для него новую коллекцию (опционально)
head, tail = os.path.split(f)
collection_name = tail.replace('.obj', '')
myCol = bpy.data.collections.new(collection_name)
bpy.context.scene.collection.children.link(myCol)

# Перемещаем импортированные объекты в новую коллекцию
for ob in bpy.context.selected_objects:
myCol.objects.link(ob)
