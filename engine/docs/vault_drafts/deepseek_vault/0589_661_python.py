class RPNTransformer(RecursivePatternNetwork):
    """Гибрид RPN + Transformer компоненты"""
def __init__(self, vocab_size, embed_dim, pattern_dim, num_classes, num_layers):
super().__init__(vocab_size, embed_dim, pattern_dim)
# Добавляем transformer-подобные слои для сравнения
self.transformer_layers = nn.ModuleList([
nn.TransformerEncoderLayer(embed_dim, nhead=8)
for _ in range(num_layers)
])
self.classifier = nn.Linear(pattern_dim, num_classes)
3. Процедура обучения:
python
