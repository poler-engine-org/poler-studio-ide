from direct.showbase.ShowBase import ShowBase

class MyApp(ShowBase):
def __init__(self):
ShowBase.__init__(self)

# Устанавливаем цвет фона
self.setBackgroundColor(0.1, 0.1, 0.3)

# Загружаем встроенную модель куба
self.box = self.loader.loadModel("models/box")
# Размещаем модель в сцене
self.box.reparentTo(self.render)
# Устанавливаем позицию модели
self.box.setPos(0, 5, 0)[reference:8]

# Настраиваем камеру, чтобы она смотрела на модель
self.camera.setPos(0, -10, 3)[reference:9]
self.camera.lookAt(self.box)

app = MyApp()
app.run()

После запуска этого скрипта вы увидите 3D-окно с синим фоном и кубом, на который направлена камера-
7
.
