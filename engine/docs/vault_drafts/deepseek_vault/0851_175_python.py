import types
import dis

def make_add_function():
# Создаём код, который складывает два числа
code = bytes([
0x7c, 0x00,  # LOAD_FAST 0 (a)
0x7c, 0x01,  # LOAD_FAST 1 (b)
0x17,        # BINARY_OP 0 (+)
0x53,        # RETURN_VALUE
])
# Константы и имена
consts = (None,)
names = ('a', 'b')
# Создаём объект кода (упрощённо, без многих полей)
code_obj = types.CodeType(
2, 0, 0, 0, 0, 0,
code, consts, names, (),
"myadd", "myadd", 1, b"", ()
)
func = types.FunctionType(code_obj, {})
return func

f = make_add_function()
print(f(3, 5))  # 8
