# Допустим, у нас есть эмбеддинг слова (dim=16)
word_embedding = np.random.randn(16)  # вместо реального

# Создаём систему
brain = POLERUnifiedSystem(dim=16)

# Подаём слово как стимул
for t in range(10):
result = brain.step(stimulus=word_embedding)
print(f"t={t}: E={result['energy']:.3f}, мысль = {result['state'][:3]}...")
