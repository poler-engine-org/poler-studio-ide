# Библиотека архетипов
archetypes = nn.Parameter(torch.randn(32, 256))

# Проекция вектора на пространство архетипов
def decompose(vector):
# Возвращает коэффициенты α₁...α₃₂
coefficients = vector @ archetypes.T  # [32]
return F.softmax(coefficients, dim=0)  # сумма = 1

coeffs_ru = decompose(latent_vectors_ru[0])  # [0.92, 0.02, 0.01, 0.05, ...]
coeffs_en = decompose(latent_vectors_en[0])  # [0.91, 0.02, 0.01, 0.06, ...]
# Первый архетип (действие_перевод) доминирует в обоих языках
ГЕНЕРАЦИЯ (предсказание следующего вектора):
python
