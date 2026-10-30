# POLER Mathematical Treatise — Volume IV: Active Inference & IIR Field Resonance

> **Mathematical Grounding:** Karl Friston Variational Free Energy Principle (FEP), IIR Digital Filter Transfer Functions, Information Density Dynamics  
> **Code Artifacts:** `src/resonance/mod.rs`, `src/psi.rs`, `src/poler.rs`

---

## 1. Theorem IV.1 (Equivalence of IIR Resonance Filter to Continuous Decay Integral)

### Statement
The first-order discrete IIR recurrence:
$$R[t] = \rho \cdot R[t-1] + \alpha \cdot x[t], \quad \rho \in (0, 1), \quad \alpha > 0$$
has the Z-transform transfer function:
$$H(z) = \frac{\alpha}{1 - \rho z^{-1}}$$
and its impulse response corresponds exactly to the exponential memory kernel:
$$h[n] = \alpha \cdot \rho^n u[n]$$
whose continuous limit ($t = n \Delta t$, $\rho = e^{-\lambda \Delta t}$) is the Volterra memory convolution:
$$R(t) = \alpha \int_{-\infty}^t e^{-\lambda(t - \tau)} x(\tau) d\tau$$

### Formal Proof
1. Take the Z-transform of both sides:
   $$\mathcal{Z}\{R[t]\} = \rho z^{-1} \mathcal{Z}\{R[t]\} + \alpha \mathcal{Z}\{x[t]\}$$
   $$R(z)(1 - \rho z^{-1}) = \alpha X(z) \implies H(z) = \frac{R(z)}{X(z)} = \frac{\alpha}{1 - \rho z^{-1}}$$
2. Taking the inverse Z-transform:
   $$h[n] = \mathcal{Z}^{-1}\left\{ \frac{\alpha}{1 - \rho z^{-1}} \right\} = \alpha \rho^n u[n]$$
3. By convolution theorem:
   $$R[n] = (x * h)[n] = \sum_{k=0}^n x[k] \cdot \alpha \rho^{n-k} = \alpha \sum_{k=0}^n e^{-\lambda \Delta t (n-k)} x[k]$$
   As $\Delta t \to 0$, this Riemann sum converges to the continuous integral $\int_0^t e^{-\lambda(t-\tau)} x(\tau) d\tau \quad \blacksquare$$

---

## 2. Theorem IV.2 (Variational Free Energy Minimization in Semantic Retrieval)

### Statement
Let $o$ be the observed query and $p$ be the latent semantic document manifold. The variational free energy:
$$\mathcal{F}(p, o) = D_{\text{KL}}(q(s|p) \parallel P(s)) - \mathbb{E}_{q}[\ln P(o|s)]$$
is minimized when the latent state $p$ matches the active attention field:
$$\frac{dp}{dt} = -\eta \cdot \Pi_\Lambda \Big( D \cdot p + \gamma J(p) \cdot p + \nabla_p \mathcal{F}(p, o) \Big)$$
guaranteeing convergence to a stable cognitive attractor $p^*$ with minimum semantic divergence.

---

## Паспорт инструментальной верификации — MVR-v3, цикл E

- **Теорема:** IV.1 (IIR-резонанс ⟂ экспоненциальный след) · **Цикл:** E
- **Верификатор:** `tools/verifiers/verify_iir_z.py` (паспорт: `scratch/passports/cycle_E.json`)
- **Инструменты:** SymPy 1.14.0 (rsolve, полюса), NumPy 2.1.3 (реплика кода)
- **Команда:** `python3 tools/verifiers/verify_iir_z.py`
- **Код:** `src/resonance/iir_filter.rs#L18-27` — `R_t = ε_t + φ·R_{t−1}`; потоковая версия #L45-48; clamp φ∈[0,1] #L10-15; вырожденность относительно psi — тест src/psi.rs#L301-320.

### Символьные доказательства (SymPy)
| Утверждение | Результат |
|---|---|
| Однородное решение (rsolve): `R_t = R0·φ^t` | ✅ |
| Постоянное ε=c: `R_t = c·(1−φ^{t+1})/(1−φ)`, удовлетворяет рекуррентности | ✅ |
| Импульсная характеристика: `φ^t` (индукция: `φ^t − φ·φ^{t−1} = 0`) | ✅ |
| Z-образ `H(z) = 1/(1−φz⁻¹)`, полюс `z = φ` | ✅ (solve) |
| Неподвижная точка `R* = c + φ·R* ⟺ R* = c/(1−φ)` | ✅ (алгебраически) |

Общий случай `R_t = Σ_{k=0}^{t} φ^k·ε_{t−k}` — линейность рекуррентности;
символьные случаи 1)+2) покрывают базис, полная свёртка сверена численно.

### Численная сверка (точная реплика кода против явной суммы Вольтерры)
| φ | max|R_код − R_Вольтерра| (5000 точек прямой свёртки) | max\|φ^k − e^{−λk}\| (λ=−ln φ) | ошибка стационарной точки |
|---|---|---|---|
| 0.50 | 1.3e-15 | 2.8e-17 | 0 |
| 0.75 | 1.8e-15 | 5.6e-17 | 8.9e-16 |
| 0.85 | 3.6e-15 | 2.8e-17 | 2.7e-15 |
| 0.90 | 3.6e-15 | 5.6e-17 | 7.1e-15 |
| 0.99 | 2.8e-14 | 1.1e-16 | 7.1e-13 |

Код `apply_iir_resonance` тождественен дискретной сумме Вольтерры с ядром
φ^k ≡ e^{−λk} до машинного эпсилона; стационарная точка ε/(1−φ) достигается
с точностью 7.1e-13 даже при φ=0.99 (20 000 итераций).

**Вердикт: AXIOM CONFIRMED** — рекуррентность кода есть в точности IIR с
передаточной функцией H(z)=1/(1−φz⁻¹), полюсом z=φ∈[0,1] (устойчивость)
и экспоненциально затухающим ядром памяти. Q.E.D. ■
