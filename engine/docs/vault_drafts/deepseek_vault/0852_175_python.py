import types

def assemble_code(archetypes, variables):
# Строим тело функции как последовательность инструкций
code_parts = []
for arch in archetypes:
code_parts.append(arch.bytecode_template % variables)
full_code = b''.join(code_parts)

# Создаём CodeType с нужными именами и константами
code_obj = types.CodeType(
argcount=len(variables['args']),
kwonlyargcount=0,
nlocals=len(variables['locals']),
stacksize=10,
flags=0,
code=full_code,
consts=variables['consts'],
names=tuple(variables['names']),
varnames=tuple(variables['locals']),
filename="<generated>",
name="my_func",
firstlineno=1,
lnotab=b'',
freevars=(),
cellvars=(),
)
func = types.FunctionType(code_obj, {})
return func
