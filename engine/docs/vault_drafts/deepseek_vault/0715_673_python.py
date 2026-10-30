import ast
from collections import Counter
import itertools

class PatternExtractor:
def __init__(self):
self.patterns = Counter()

def extract_from_file(self, filename):
with open(filename) as f:
tree = ast.parse(f.read())

# Извлекаем паттерны типа "if-elif-else", "for-append", etc.
for node in ast.walk(tree):
pattern = self._node_to_pattern(node)
self.patterns[pattern] += 1

def get_top_patterns(self, n=100):
return self.patterns.most_common(n)

# Результат: [("if-else", 0.18), ("for-range", 0.12), ...]
