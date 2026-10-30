import jsonschema
import json

def validate_skill_output(output: dict, schema: dict) -> bool:
try:
jsonschema.validate(instance=output, schema=schema)
return True
except jsonschema.ValidationError as e:
print(f"Validation error: {e}")
return False

# Пример использования в Executor
schema = {
"type": "object",
"properties": {
"status": {"type": "string"},
"confidence": {"type": "number", "minimum": 0, "maximum": 1},
"data": {"type": "object"}
},
"required": ["status", "confidence"]
}

# После получения данных от скрипта
if not validate_skill_output(output, schema):
raise ValueError("Invalid output from skill")
