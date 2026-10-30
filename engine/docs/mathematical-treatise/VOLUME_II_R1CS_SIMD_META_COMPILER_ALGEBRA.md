# POLER Mathematical Treatise — Volume II: R1CS Circuit & AVX2 SIMD Algebra

> **Mathematical Grounding:** Boolean Mask Ring, Isomorphism of Ternary Coefficients, Common Subexpression Deduplication Graph  
> **Code Artifacts:** `src/quantum/meta_compiler.rs` (lines 80–350), `src/quantum/crystallizer.rs`

---

## 1. Theorem II.1 (Branchless Bit-Level Mask Equivalence for Ternary Coefficients)

### Statement
Let $x \in \mathbb{R}$ represented in IEEE-754 single-precision float (`f32`). Let $c \in \{-1, 0, +1\}$. Define the bit-level operations:
$$\text{AbsorbMask}(c) = \begin{cases} \texttt{0x00000000}, & c = 0 \\ \texttt{0xFFFFFFFF}, & c \in \{-1, +1\} \end{cases}$$
$$\text{SignMask}(c) = \begin{cases} \texttt{0x80000000}, & c = -1 \\ \texttt{0x00000000}, & c \in \{0, +1\} \end{cases}$$
The branchless transformation:
$$f(x, c) = (x \ \& \ \text{AbsorbMask}(c)) \oplus \text{SignMask}(c)$$
is algebraically exact:
$$\forall x \in \mathbb{R}, \forall c \in \{-1, 0, +1\}, \quad f(x, c) = c \cdot x$$

### Formal Bitwise Proof
1. **Case $c = 0$:**
   $$\text{AbsorbMask}(0) = \texttt{0x00000000}, \quad \text{SignMask}(0) = \texttt{0x00000000}$$
   $$f(x, 0) = (x \ \& \ \texttt{0x00000000}) \oplus \texttt{0x00000000} = \texttt{0x00000000} = +0.0 = 0 \cdot x$$
2. **Case $c = +1$:**
   $$\text{AbsorbMask}(+1) = \texttt{0xFFFFFFFF}, \quad \text{SignMask}(+1) = \texttt{0x00000000}$$
   $$f(x, +1) = (x \ \& \ \texttt{0xFFFFFFFF}) \oplus \texttt{0x00000000} = x \oplus 0 = x = 1 \cdot x$$
3. **Case $c = -1$:**
   $$\text{AbsorbMask}(-1) = \texttt{0xFFFFFFFF}, \quad \text{SignMask}(-1) = \texttt{0x80000000}$$
   $$f(x, -1) = (x \ \& \ \texttt{0xFFFFFFFF}) \oplus \texttt{0x80000000} = x \oplus \texttt{0x80000000}$$
   In IEEE-754 format, the most significant bit (bit 31) is the sign bit $S$. Toggling bit 31 via XOR invert the sign of the floating-point number without modifying the 8 exponent bits or 23 mantissa bits:
   $$(-1)^S \cdot 2^{E-127} \cdot (1 + M) \xrightarrow{\oplus \texttt{0x80000000}} (-1)^{S \oplus 1} \cdot 2^{E-127} \cdot (1 + M) = -x = (-1) \cdot x \quad \blacksquare$$

### SIMD Vectorization (`VectorGate8`)
For 8 lanes simultaneously across 256-bit AVX2 registers:
$$\mathbf{r} = \text{_mm256_add_ps}\Big( \text{_mm256_xor_ps}(\text{_mm256_and_ps}(\mathbf{l}, \mathbf{M}_{\text{abs},l}), \mathbf{M}_{\text{sign},l}), \; \text{_mm256_xor_ps}(\text{_mm256_and_ps}(\mathbf{k}, \mathbf{M}_{\text{abs},r}), \mathbf{M}_{\text{sign},r}) \Big)$$
Requires **zero multiplication units** and **zero runtime branching**, executing in **1 clock cycle** throughput on Zen 3/4/5 and Intel Skylake/RaptorLake AVX2 pipelines.

---

## Паспорт инструментальной верификации — MVR-v3, цикл B

- **Теорема:** II.1 (безветвежная масочная эквивалентность c ∈ {−1, 0, +1}) · **Цикл:** B
- **Верификатор:** `tools/verifiers/verify_vg8_masks.py` (паспорт: `scratch/passports/cycle_B.json`)
- **Инструменты:** Z3 5.1.0 (SMT, теория IEEE-754 FP), NumPy 2.1.3 (побитовый дифференциал)
- **Команда:** `python3 tools/verifiers/verify_vg8_masks.py`
- **Константы сверены с кодом:** `MASK_KEEP=0xFFFFFFFF`, `MASK_KILL=0x0`, `MASK_FLIP=0x80000000` (meta_compiler.rs#L83-89), `masks_of()` #L151-158, горячий путь `xor(and(l,al),sl)` #L897-898.

### SMT-доказательства (Z3, float32-теория)
| # | Утверждение | Результат |
|---|---|---|
| II.1a | c=+1: `(x & 0xFFFFFFFF) ⊕ 0 == x` — весь домен битовых паттернов | **UNSAT** (доказано) |
| II.1b | c=−1: `x ⊕ 0x80000000 == fp.neg(x)` для всех не-NaN (вкл. ±0, ±Inf) | **UNSAT** (доказано) |
| II.1c | c=0: IEEE-числовое равенство `+0.0 ~fp.eq~ 0·x` для конечных x | **UNSAT** (доказано) |
| II.1c′ | Битовый кавет: для конечных x<0 маска даёт +0.0, скаляр −0.0 | **SAT** — кавет реален, значения IEEE-равны |
| II.1d | Домен: для x ∈ {NaN, ±Inf} c=0 рушится (0·Inf = NaN ≠ +0.0) | **SAT** — домен обязан быть конечным |

⚠ Семантический нюанс, найденный инструментом: `=` на FP-сорте Z3 — БИТОВОЕ
равенство (+0 ≠ −0); IEEE-числовое равенство — `fp.eq` (+0 == −0). Теорема
II.1 верна в fp.eq-семантике; битовый кавет нуля — ровно то, что фиксирует
тест meta_compiler.rs#L1372 (`g.to_bits() == w.to_bits() || (g==0.0 && w==0.0)`).

### Побитовый дифференциал (NumPy: 2 000 018 образцов)
Краевые случаи IEEE-754 (±0, ±min/max denormal, ±min/max normal, ±Inf, ±NaN, ±π)
+ 2·10⁶ структурированных случайных битовых паттернов (нормали, окрестность 1,
денормали, хаос) против честного скалярного умножения f32:

| c | Побитово точных | Расхождение значений (конечный домен) | Только знак нуля |
|---|---|---|---|
| +1 | 1 999 077 | **0** | 0 |
| −1 | 1 998 027 | **0** | 0 |
| 0 | 873 970 | **0** | 1 124 055 — все до единицы предсказаны знаком источника (x<0), все являются парой ±0.0 |

(NaN-паттерны вне домена: c=−1 флипает бит знака NaN — соответствует fp.neg.)

### Заявка «1 такт» (SIMD)
Структурно верно: ноль умножений, ноль ветвлений (`and+xor+add` на YMM —
meta_compiler.rs#L897-898, кодоген #L1104-1122). Аппаратный замер тактов —
PENDING (cargo-asm в STANDBY, см. MVR_PROTOCOL.md §3).

**Вердикт: CONFIRMED WITH CAVEATS** — домен конечных x; кавет +0.0/−0.0 для
c=0 и x<0 (IEEE-равны, биты различаются — уже закрыт тестом L1372).
Q.E.D. ■
