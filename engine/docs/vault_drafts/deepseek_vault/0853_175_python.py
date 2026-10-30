import ctypes

pythonapi = ctypes.pythonapi
PyCode_New = pythonapi.PyCode_New
PyCode_New.argtypes = ...
# Создаём PyCodeObject с полным контролем
