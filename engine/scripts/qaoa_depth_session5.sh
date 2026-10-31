#!/usr/bin/env bash
# СЕССИЯ 5 — эксперимент 1: ГЛУБИНА КУТРИТНОГО QAOA p=1 → p=2 → p=3
# На живом K4-ядре коннектома мухи (те же 6 нейронов/11 рёбер, что в сессии 4).
#
# ЧТО НОВОГО благодаря исправленным граблям:
#   1. eye(81) напрямую — без грабель kron(eye(9),eye(9))
#   2. CSE-кеш: вложенные выражения больше НЕ взрываются 2^N — скан углов
#      любой глубины идёт секундами (в сессии 4 p=1 висел минутами)
#   3. Многослойный анзац — послойные let: T1 = слой1(flat), T2 = слой2(T1)…
#
# Метрики: E[cut](p); Born-концентрация: P(best), top-10 mass, H, exp(H), 1/Σp².
set -u
OUT=/home/z/my-project/download/experiments/qaoa_depth_v063.txt
GEN=/home/z/my-project/scripts/gen_qaoa_depth.py
SESSDIR=/home/z/my-project/scripts
mkdir -p "$SESSDIR" "$(dirname "$OUT")"

cat > "$OUT" << 'HDR'
════════════════════════════════════════════════════════════════
 СЕССИЯ 5: ГЛУБИНА КУТРИТНОГО QAOA p=1 → p=2 → p=3
 K4-ядро коннектома мухи, 6 кутритов, 3^6=729, движок v0.63.0+CSE
════════════════════════════════════════════════════════════════
Грабли сессии 4 сняты: eye(81) напрямую; CSE-кеш подвыражений —
вложенные анзацы больше не удваиваются на каждом ребре (2^11 → ~22 матопов).
Анзац p слоёв: |ψ_p⟩ = M_p·C_p·…·M_1·C_1·|flat⟩, послойные let.
Стратегия углов: координатный подъём (слои по очереди, сетка 8γ×6β).
Оптимум (брутфорс 729): 4403/4435 = 99.3% рёбер; случайный ≈ 1478.
HDR

scan_vals() {  # $1 = файл вывода, $2 = сколько последних чисел
python3 - "$1" "$2" << 'PYEOF'
import sys
vals, errs = [], 0
for ln in open(sys.argv[1], errors='replace'):
    if ln.startswith('poler> '):
        b = ln[7:].strip()
        if b.startswith('❌'):
            errs += 1; continue
        try: vals.append(float(b.split(' ')[0].rstrip('i')))
        except ValueError: pass
n = int(sys.argv[2])
for v in vals[-n:]:
    print(f"{v:.6f}")
if errs:
    print(f"ERRS={errs}", file=sys.stderr)
PYEOF
}

best_of_grid() {  # $1 = файл значений (48 строк) → печатает "G B VAL"
python3 - "$1" << 'PYEOF'
import sys, itertools
vals = [float(x) for x in open(sys.argv[1]).read().split()]
GAM = [0.3,0.6,0.9,1.2,1.5,1.8,2.4,3.0]; BET = [0.3,0.6,0.9,1.2,1.6,2.0]
grid = list(itertools.product(GAM, BET))
pairs = sorted(zip(grid, vals), key=lambda kv: -kv[1])
(g, b), v = pairs[0]
print(f"{g} {b} {v:.6f}")
PYEOF
}

# ---------- БЛОК 1: p=1 полный скан (якорь сессии 4) ----------
echo "" >> "$OUT"
echo "── БЛОК 1: p=1 полный скан 48 точек (якорь сессии 4: γ=0.9 β=0.9 → 3535) ──" >> "$OUT"
python3 "$GEN" scan 1 > "$SESSDIR/qaoa5_p1.session"
T0=$(date +%s.%N)
poler-engine --shell < "$SESSDIR/qaoa5_p1.session" > /tmp/qaoa5_p1.txt 2>&1
T1=$(date +%s.%N); P1TIME=$(echo "$T1 - $T0" | bc)
scan_vals /tmp/qaoa5_p1.txt 48 > /tmp/qaoa5_p1_vals.txt
B1L=$(best_of_grid /tmp/qaoa5_p1_vals.txt)
G1=$(echo "$B1L" | awk '{print $1}'); B1=$(echo "$B1L" | awk '{print $2}'); V1=$(echo "$B1L" | awk '{print $3}')
echo "p=1: максимум E[cut'] = $V1 = $(python3 -c "print(f'{$V1*1496:.0f}')")/4435 синапсов при γ=$G1 β=$B1" >> "$OUT"
echo "  (якорь сессии 4: 2.3631 = 3535 при γ=0.9 β=0.9)" >> "$OUT"

# ---------- БЛОК 2: p=2 координатный подъём ----------
echo "" >> "$OUT"
echo "── БЛОК 2: p=2 — слой2 скан (слой1 = $G1,$B1), затем рефайн слоя 1 ──" >> "$OUT"
python3 "$GEN" scan 2 "$G1" "$B1" > "$SESSDIR/qaoa5_p2a.session"
T0=$(date +%s.%N)
poler-engine --shell < "$SESSDIR/qaoa5_p2a.session" > /tmp/qaoa5_p2a.txt 2>&1
T1=$(date +%s.%N); P2ATIME=$(echo "$T1 - $T0" | bc)
scan_vals /tmp/qaoa5_p2a.txt 48 > /tmp/qaoa5_p2a_vals.txt
B2A=$(best_of_grid /tmp/qaoa5_p2a_vals.txt)
G2=$(echo "$B2A" | awk '{print $1}'); B2=$(echo "$B2A" | awk '{print $2}'); V2A=$(echo "$B2A" | awk '{print $3}')
echo "p=2 проход A (слой2): E[cut'] = $V2A = $(python3 -c "print(f'{$V2A*1496:.0f}')")/4435 при γ₂=$G2 β₂=$B2" >> "$OUT"

python3 "$GEN" scan12 "$G2" "$B2" > "$SESSDIR/qaoa5_p2b.session"
T0=$(date +%s.%N)
poler-engine --shell < "$SESSDIR/qaoa5_p2b.session" > /tmp/qaoa5_p2b.txt 2>&1
T1=$(date +%s.%N); P2BTIME=$(echo "$T1 - $T0" | bc)
scan_vals /tmp/qaoa5_p2b.txt 48 > /tmp/qaoa5_p2b_vals.txt
B2B=$(best_of_grid /tmp/qaoa5_p2b_vals.txt)
G1F=$(echo "$B2B" | awk '{print $1}'); B1F=$(echo "$B2B" | awk '{print $2}'); V2=$(echo "$B2B" | awk '{print $3}')
echo "p=2 проход B (рефайн слоя1): E[cut'] = $V2 = $(python3 -c "print(f'{$V2*1496:.0f}')")/4435 при γ₁=$G1F β₁=$B1F (γ₂=$G2 β₂=$B2)" >> "$OUT"

# ---------- БЛОК 3: p=3 — слой3 поверх лучшего p=2 ----------
echo "" >> "$OUT"
echo "── БЛОК 3: p=3 — слой3 скан поверх лучшего p=2 ──" >> "$OUT"
python3 "$GEN" scan 3 "$G1F" "$B1F" "$G2" "$B2" > "$SESSDIR/qaoa5_p3.session"
T0=$(date +%s.%N)
poler-engine --shell < "$SESSDIR/qaoa5_p3.session" > /tmp/qaoa5_p3.txt 2>&1
T1=$(date +%s.%N); P3TIME=$(echo "$T1 - $T0" | bc)
scan_vals /tmp/qaoa5_p3.txt 48 > /tmp/qaoa5_p3_vals.txt
B3L=$(best_of_grid /tmp/qaoa5_p3_vals.txt)
G3B=$(echo "$B3L" | awk '{print $1}'); B3B=$(echo "$B3L" | awk '{print $2}'); V3=$(echo "$B3L" | awk '{print $3}')
echo "p=3: E[cut'] = $V3 = $(python3 -c "print(f'{$V3*1496:.0f}')")/4435 при γ₃=$G3B β₃=$B3B (слои 1-2: $G1F,$B1F / $G2,$B2)" >> "$OUT"

# ---------- БЛОК 4: Born-концентрация ----------
echo "" >> "$OUT"
echo "── БЛОК 4: Born-концентрация p=1 / p=2 / p=3 (лучшие углы) ──" >> "$OUT"

extract_born() {  # $1=p, дальше углы
python3 - "$@" << 'PYEOF'
import sys, math, subprocess
p = int(sys.argv[1])
angles = [float(x) for x in sys.argv[2:2+2*p]]
cmd = ["python3", "/home/z/my-project/scripts/gen_qaoa_depth.py", "state", str(p)] + [str(a) for a in angles]
sess = subprocess.run(cmd, capture_output=True, text=True).stdout
r = subprocess.run(["poler-engine", "--shell"], input=sess, capture_output=True, text=True)
amp_line = None
for ln in r.stdout.splitlines():
    if ln.startswith("poler> ["):
        amp_line = ln[len("poler> "):].strip()
if not amp_line:
    print("ОШИБКА: вектор состояния не найден"); sys.exit(1)
inner = amp_line.strip("[]")
amps = []
for part in inner.split(";"):
    t = part.strip().replace(" ", "")
    if t.endswith("i"):
        core = t[:-1]
        if "+" in core:
            re_, im_ = core.rsplit("+", 1)
        else:
            i = core.find("-", 1)
            re_, im_ = core[:i], core[i:]
        amps.append(complex(float(re_), float(im_)))
    else:
        amps.append(complex(float(t), 0.0))
probs = [abs(z)**2 for z in amps]
tot = sum(probs)
probs = [q/tot for q in probs]
raw = [(0,1,735),(0,2,24),(0,4,1496),(0,5,1222),(1,2,251),(1,3,231),(1,4,206),(1,5,194),(2,4,40),(2,5,28),(4,5,8)]
cuts = []
for x in range(729):
    c = [(x // 3**j) % 3 for j in range(6)]
    cuts.append(sum(w for (u,v,w) in raw if c[u] != c[v]))
best_cut = max(cuts)
best_idx = cuts.index(best_cut)
order = sorted(range(729), key=lambda x: -probs[x])
H = -sum(q*math.log(q) for q in probs if q > 1e-300)
pr = 1.0/sum(q*q for q in probs)
print(f"p={p}: амплитуд {len(amps)}, Σ|ψ|² = {tot:.6f}")
print(f"  P(оптимум cut=4403, x={best_idx}) = {probs[best_idx]:.4f}")
print(f"  P(top-1 Born) = {probs[order[0]]:.4f} (x={order[0]}, cut={cuts[order[0]]})")
print(f"  top-10 масса = {sum(probs[x] for x in order[:10]):.4f}")
print(f"  энтропия H = {H:.3f} нат → эффективная размерность exp(H) = {math.exp(H):.1f}")
print(f"  участие 1/Σp² = {pr:.1f}")
PYEOF
}

{
extract_born 1 "$G1" "$B1"
extract_born 2 "$G1F" "$B1F" "$G2" "$B2"
extract_born 3 "$G1F" "$B1F" "$G2" "$B2" "$G3B" "$B3B"
} >> "$OUT" 2>&1

# ---------- БЛОК 5: тайминги + итог ----------
{
echo ""
echo "── БЛОК 5: тайминги CSE (сессия 4: p=1 скан висел минутами) ──"
echo "p=1 скан 48 точек:   ${P1TIME}s"
echo "p=2 проход A (48):   ${P2ATIME}s"
echo "p=2 проход B (48):   ${P2BTIME}s"
echo "p=3 слой3 скан (48): ${P3TIME}s"
echo ""
echo "ИТОГ ГЛУБИНЫ:"
echo "  p=1: E[cut] = $(python3 -c "print(f'{$V1*1496:.0f}')")/4435 ($(python3 -c "print(f'{100*$V1*1496/4435:.1f}')")%)"
echo "  p=2: E[cut] = $(python3 -c "print(f'{$V2*1496:.0f}')")/4435 ($(python3 -c "print(f'{100*$V2*1496/4435:.1f}')")%)"
echo "  p=3: E[cut] = $(python3 -c "print(f'{$V3*1496:.0f}')")/4435 ($(python3 -c "print(f'{100*$V3*1496/4435:.1f}')")%)"
echo "  оптимум: 4403/4435 (99.3%)"
} >> "$OUT"

echo "готово: $OUT"
