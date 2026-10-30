def visualize_pattern_hierarchy(model, text):
    """Визуализация иерархии паттернов"""
analysis = model.analyze_patterns(text, tokenizer)

# Строим граф паттернов
graph = nx.DiGraph()
for level, patterns in analysis['pattern_hierarchy'].items():
for pattern in patterns:
graph.add_node(pattern.id,
type=pattern.pattern_type,
level=level,
activation=pattern.activation)

# Визуализируем
pos = nx.spring_layout(graph)
nx.draw(graph, pos, node_color=[
pattern['activation'] for pattern in graph.nodes.values()
])
plt.show()
3. Адаптивность:
python
