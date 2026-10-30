# Вместо плотных матриц использовать разреженные (если dim > 1000)
from scipy import sparse

# Для GPU-ускорения добавить декораторы numba или cupy
from numba import jit, prange

# Пример JIT-функции для шага динамики:
@jit(nopython=True, parallel=True)
def _fast_canonical_step(p, D, J, O, eta, gamma, lambda_O):
grad = D @ p + gamma * (J @ p) + lambda_O * (O @ p)
return p - eta * grad
