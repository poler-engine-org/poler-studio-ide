
---

## 🧠 2️⃣ **`POLER_Psi_v3.py`**
(код ядра з класами POLER System)

```python
import numpy as np

class Perception:
    def __init__(self, G=None):
        self.G = G if G is not None else np.eye(2)

    def omega(self, o_t):
        return np.tanh(o_t)

class LogicProjector:
    def __init__(self, Jc=None):
        self.Jc = Jc if Jc is not None else np.zeros((1,2))

    def Pi(self):
        Jc = self.Jc
        if np.all(Jc == 0):
            return np.eye(2)
        return np.eye(2) - Jc.T @ np.linalg.inv(Jc @ Jc.T) @ Jc

class Resonance:
    def __init__(self, rho=0.9, alpha=1.0):
        self.rho = rho
        self.alpha = alpha

    def weights(self, n):
        return np.array([self.alpha * self.rho**k for k in range(1, n+1)])

class PsiField:
    def __init__(self, gamma=0.5):
        self.gamma = gamma

    def evolve(self, p_t, o_seq, G, Pi, eta=0.05):
        n = len(o_seq) - 1
        w = np.array([0.9**k for k in range(1, n+1)])
        grad_F = 2 * G @ (p_t - o_seq[-1])
        grad_eps = np.sum([w[k-1] * (p_t - o_seq[-k-1]) for k in range(1, n+1)], axis=0)
        dp = Pi @ (-grad_F + self.gamma * grad_eps)
        return p_t + eta * dp

if __name__ == "__main__":
    Ω = Perception()
    Π = LogicProjector()
    Ψ = PsiField()
    G = np.eye(2)

    o_seq = [np.random.randn(2) for _ in range(5)]
    p = np.zeros(2)

    for t in range(10):
        p = Ψ.evolve(p, o_seq, G, Π.Pi())
        print(f"t={t}, p={p}")
