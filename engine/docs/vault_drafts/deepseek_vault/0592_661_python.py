# Линейная сложность O(n log n)
def complexity_analysis():
    """RPN имеет почти линейную сложность"""
lengths = [100, 1000, 10000, 100000]
for length in lengths:
tokens = torch.randint(0, 30000, (1, length))
start = time.time()
model(tokens)
elapsed = time.time() - start
print(f"Length {length}: {elapsed:.4f}s (O~{length * np.log(length):.0f})")
2. Интерпретируемость:
python
