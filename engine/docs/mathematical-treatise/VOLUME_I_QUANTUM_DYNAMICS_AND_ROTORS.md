# POLER Mathematical Treatise — Volume I: Quantum-Semantic Dynamics & Rotor J

> **Mathematical Grounding:** Unitary Lie Algebra $\mathfrak{so}(n)$, Skew-Symmetric Rotors, Born Lottery Measure, Bloch Phase Sphere  
> **Code Artifacts:** `crates/pqc/src/gyro.rs` (J = A − Aᵀ, precess_step), `crates/pqc/src/born.rs` (гейт нормировки), `src/quantum/mod.rs` (мост к учебной программе)
> **MVR-v3:** цикл D — паспорт инструментальной верификации в конце тома
> (SymPy 1.14.0 символьно; NumPy 2.1.3 RK4/спектр; Rust-тест lockstep)

---

## 1. Theorem I.1 (Energy Conservation in the Non-Dissipative Semantic Rotor)

### Statement
Let $\mathbf{\psi}(t) \in \mathbb{R}^n$ be the semantic state vector on the sphere $S^{n-1}$. Let $U \in \mathbb{R}^{n \times n}$ be an arbitrary projection matrix extracted from the model weights ($W_q, W_k$). The canonical rotor operator:
$$J = U - U^T$$
generates a purely energy-preserving, unitary trajectory under Hamiltonian flow:
$$\frac{d\mathbf{\psi}}{dt} = J\mathbf{\psi}$$
such that the $L_2$ norm $\|\mathbf{\psi}(t)\|^2$ is strictly invariant:
$$\forall t \ge 0, \quad \frac{d}{dt}\|\mathbf{\psi}(t)\|^2 = 0 \implies \|\mathbf{\psi}(t)\|^2 = \|\mathbf{\psi}(0)\|^2$$

### Formal Proof
1. Compute the time derivative of the squared Euclidean norm:
   $$\frac{d}{dt}\|\mathbf{\psi}(t)\|^2 = \frac{d}{dt} \left( \mathbf{\psi}^T \mathbf{\psi} \right) = \dot{\mathbf{\psi}}^T \mathbf{\psi} + \mathbf{\psi}^T \dot{\mathbf{\psi}}$$
2. Substitute the dynamical equation $\dot{\mathbf{\psi}} = J\mathbf{\psi}$:
   $$\frac{d}{dt}\|\mathbf{\psi}\|^2 = (J\mathbf{\psi})^T \mathbf{\psi} + \mathbf{\psi}^T (J\mathbf{\psi}) = \mathbf{\psi}^T J^T \mathbf{\psi} + \mathbf{\psi}^T J \mathbf{\psi} = \mathbf{\psi}^T (J^T + J) \mathbf{\psi}$$
3. Examine the matrix $J^T + J$:
   $$J^T = (U - U^T)^T = U^T - (U^T)^T = U^T - U = -(U - U^T) = -J$$
   Therefore:
   $$J^T + J = -J + J = \mathbf{0}_{n \times n}$$
4. Substitute $\mathbf{0}$ into the quadratic form:
   $$\frac{d}{dt}\|\mathbf{\psi}\|^2 = \mathbf{\psi}^T \mathbf{0} \mathbf{\psi} = 0 \quad \blacksquare$$

### Code Grounding & Invariant Check
> ⚠ MVR-v3, фаза 3 (поправка): в первой редакции тома здесь значился
> несуществующий тест `test_rotor_energy_conservation` — найден и убран
> инструментальной сверкой (grep по репо: 0 вхождений). Реальные якоря:

```rust
// crates/pqc/src/gyro.rs#L279-L298 — J = A − Aᵀ парами (верхний треугольник):
//   let j = w_ab - w_ba;          // J_ij = w, J_ji = −w (кососимметрия)
// crates/pqc/src/born.rs#L19 — гейт нормы состояния:
//   |‖ψ‖ − 1| ≤ 1e-9, иначе PqcError::NotNormalized
// crates/pqc/src/gyro.rs — тест precess_step_edge_lockstep_preserves_pair_difference
//   (добавлен циклом D MVR-v3: формульная сверка + lockstep-инвариант ребра)
```

⚠ ВАЖНОЕ РАЗГРАНИЧЕНИЕ (найдено циклом D): `precess_step` (gyro.rs#L632-645)
реализует НАПРАВЛЕННЫЙ Курамото-транспорт фаз — оба конца ребра получают
ОДИН И ТОТ ЖЕ вклад `−torque` (L639-640, комментарий «для j — тот же вклад,
J_ji = −w»). Это фазовая динамика на торе, а НЕ линейный ротор ψ̇ = Jψ:
у неё НЕТ глобального инварианта Σθ (измерено: дрейф −10.94 рад за 1000 шагов
на тест-конфигурации), тогда как линейный ротор сохраняет ‖ψ‖² точно
(теорема I.1). Единица инварианта направленного транспорта — РЕБРО:
разность θ_j − θ_i не трогается собственным моментом пары (lockstep).

---

## 2. Theorem I.2 (Born Lottery Collapse as a Valid Probability Measure)

### Statement
Given the complex or real amplitude field $\psi = [\psi_1, \psi_2, \dots, \psi_V]^T \in \mathbb{R}^V$ over vocabulary $V$ with $\|\mathbf{\psi}\| > 0$, the Born lottery sampling distribution:
$$P(k) = \frac{\psi_k^2}{\sum_{j=1}^V \psi_j^2}$$
satisfies Kolmogorov's Axioms of Probability:
1. **Non-negativity:** $\forall k, \quad P(k) \ge 0$
2. **Unit Measure:** $\sum_{k=1}^V P(k) = 1$
3. **Countable Additivity:** For disjoint subsets $A, B \subseteq V$, $P(A \cup B) = P(A) + P(B)$.

### Proof
1. $\psi_k \in \mathbb{R} \implies \psi_k^2 \ge 0$. Since $\|\mathbf{\psi}\|^2 = \sum_{j=1}^V \psi_j^2 > 0$, the ratio $P(k) = \frac{\psi_k^2}{\|\mathbf{\psi}\|^2} \ge 0$.
2. $\sum_{k=1}^V P(k) = \sum_{k=1}^V \frac{\psi_k^2}{\|\mathbf{\psi}\|^2} = \frac{1}{\|\mathbf{\psi}\|^2} \sum_{k=1}^V \psi_k^2 = \frac{\|\mathbf{\psi}\|^2}{\|\mathbf{\psi}\|^2} = 1$.
3. Linearity of summation over discrete disjoint index sets directly yields countable additivity $\blacksquare$.

---

## 3. Theorem I.3 (Langmuir-Michaelis-Menten Attention Saturation)

### Statement
The bounded non-linear activation threshold:
$$\theta(c) = \frac{V_{\max} \cdot c}{K_m + c}, \quad c \ge 0$$
is strictly monotonic, concave, and asymptotically bounded by $V_{\max}$:
$$\lim_{c \to \infty} \theta(c) = V_{\max}, \quad \frac{d\theta}{dc} > 0, \quad \frac{d^2\theta}{dc^2} < 0$$

### Proof
1. First derivative:
   $$\frac{d\theta}{dc} = \frac{V_{\max}(K_m + c) - V_{\max} c}{(K_m + c)^2} = \frac{V_{\max} K_m}{(K_m + c)^2} > 0 \quad (\text{for } V_{\max}, K_m > 0)$$
2. Second derivative:
   $$\frac{d^2\theta}{dc^2} = \frac{-2 V_{\max} K_m}{(K_m + c)^3} < 0 \quad \blacksquare$$

---

## Паспорт инструментальной верификации — MVR-v3, цикл D

- **Теорема:** I.1 (сохранение нормы линейным ротором) · **Цикл:** D
- **Верификатор:** `tools/verifiers/verify_rotor_norm.py` (паспорт: `scratch/passports/cycle_D.json`)
- **Инструменты:** SymPy 1.14.0 (CAS), NumPy 2.1.3 (RK4/спектр), rustc (тест в gyro.rs)
- **Команда:** `python3 tools/verifiers/verify_rotor_norm.py`

### Доказательство (SymPy, символьно, n=4, произвольные символьные U и ψ)
| Тождество | Результат |
|---|---|
| `simplify(Jᵀ + J) == 0` при `J = U − Uᵀ` | ✅ нулевая матрица |
| `ψᵀ(Jᵀ+J)ψ ≡ 0` (квадратичная форма) | ✅ тождественный нуль |
| `(Jψ)ᵀψ + ψᵀ(Jψ) ≡ 0` (производная ‖ψ‖²) | ✅ тождественный нуль |
| `tr(J) = 0`, `J[i][i] = 0 ∀i` | ✅ |

### Численная валидация (NumPy, n=64, случайный гауссов U)
- Спектр: `max|Re λ(J)| = 8.3e-16` против `max|Im λ| = 20.64` → **чисто мнимый** (алгебра so(n) подтверждена инструментом).
- RK4, T=50, h=0.01: дрейф ‖ψ‖² = 6.2e-4; **контроль** (J′ = U, симметричная часть жива): дрейф = **∞ (расходимость)**.
- Порядок сходимости дрейфа: h∈{0.02, 0.01, 0.005} → дрейфы {3.8e-3, 1.2e-4, 3.9e-6}, измеренный порядок **4.98 ≥ 4** — дрейф есть O(h⁴)-ошибка ИНТЕГРАТОРА, инвариант непрерывной системы точен.

### Код-грундинг (побитовая сверка, фаза 3)
- `crates/pqc/src/gyro.rs#L279-298`: J строится парами `j = w_ab − w_ba` — кососимметрия на уровне конструкции.
- `crates/pqc/src/born.rs#L19`: гейт `|‖ψ‖ − 1| ≤ 1e-9` — норма состояния проверяется кодом.
- **Находка цикла (implementation-gap кейс):** заявленный в ранней редакции инвариант `Σθ` для `precess_step` ОПРОВЕРГНУТ Rust-тестом на реальном коде: L640 `delta[j] -= torque` — направленный lockstep-транспорт, Σθ-дрейф −10.94 рад/1000 шагов. Верификатор исправлен, ложная заявка снята, истинный инвариант (разность фаз ребра, lockstep) доказан и зафиксирован тестом `precess_step_edge_lockstep_preserves_pair_difference` (75 тестов gyro зелёные).
- Формула против независимой плотной матрицы J: **0 расхождений** на 1000 случайных конфигураций.

**Вердикт: AXIOM CONFIRMED** — для линейного ротора J = U − Uᵀ (символьно + численно + спектрально); фазовый транспорт precess_step корректно описан своими собственными свойствами (lockstep), без приписывания ему чужих инвариантов. Q.E.D. ■
