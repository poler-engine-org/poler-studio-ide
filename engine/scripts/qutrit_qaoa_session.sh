#!/usr/bin/env bash
# КУТРИТНЫЙ QAOA (Max-3-Cut) на K4-ядре коннектома мухи — сессия 4, эксперимент B
# 6 нейронов = 6 кутритов, 3^6 = 729-мерный statevector, ЧИСТЫЙ Калькулятор движка.
# Реальные синаптические веса (нормировка /1496). Миксер эрмитов: X3+X3† (X†≠X, сессия 3).
set -u
OUT=/home/z/my-project/download/experiments/qutrit_qaoa_v063.txt
mkdir -p "$(dirname "$OUT")"
SESSION=/home/z/my-project/scripts/qutrit_qaoa.session

cat > "$OUT" << 'HDR'
════════════════════════════════════════════════════════════════
 КУТРИТНЫЙ QAOA: Max-3-Cut на живом коннектоме мухи (сессия 4)
 6 нейронов K4-ядра = 6 кутритов, 3^6 = 729 амплитуд, движок v0.63.0
════════════════════════════════════════════════════════════════
Нейроны: 0=79529(хаб) 1=47410(ГАМК-лейтенант) 2=55498(окт)
         3=101516(ГАМК-a) 4=74111(АХ-a) 5=106315(АХ-b)
Рёбра (реальные веса, сумма=4435): 0-1:735 0-2:24 0-4:1496 0-5:1222
  1-2:251 1-3:231 1-4:206 1-5:194 2-4:40 2-5:28 4-5:8
K4 = {0,2,4,5} — командное ядро (все 6 пар связаны). Веса нормированы w'=w/1496.
Миксер: expm(-i*beta*(X3+X3†)) — эрмитов (у кутритов X†≠X, сессия 3).
Грабли: eye(n) только n<=64 → eye(81)=kron(eye(9),eye(9)); имя E коллизирует
с константой e; в грамматике нет меток '100:' — разбор позиционный.
HDR

# ---------- БЛОК 0: кубитный QAOA на том же ядре (нативная команда) ----------
echo "── БЛОК 0: нативный кубитный QAOA MaxCut (2 кластера) на том же ядре ──" >> "$OUT"
echo "Для сравнения: 2-кластерная картина (кубиты, без весов, native quantum qaoa):" >> "$OUT"
poler-engine --exec 'quantum qaoa --edges 0-1,0-2,0-4,0-5,1-2,1-3,1-4,1-5,2-4,2-5,4-5 --p 2' >> "$OUT" 2>&1
echo "" >> "$OUT"

# ---------- Общий заголовок сессии (проекторы) ----------
COMMON='set format simple
calc let P0 = [1,0,0;0,0,0;0,0,0]
calc let P1 = [0,0,0;0,1,0;0,0,0]
calc let P2 = [0,0,0;0,0,0;0,0,1]
calc let I3 = eye(3)
calc let I9 = eye(9)
calc let I27 = eye(27)
calc let I81 = kron(eye(9), eye(9))
calc let flat3 = (1/sqrt(3))*[1;1;1]
calc let flat = kron(flat3, kron(flat3, kron(flat3, kron(flat3, kron(flat3, flat3)))))
calc let EQU01 = kron(P0,kron(P0,I81)) + kron(P1,kron(P1,I81)) + kron(P2,kron(P2,I81))
calc let EQU02 = kron(P0,kron(I3,kron(P0,I27))) + kron(P1,kron(I3,kron(P1,I27))) + kron(P2,kron(I3,kron(P2,I27)))
calc let EQU04 = kron(P0,kron(I9,kron(P0,I9))) + kron(P1,kron(I9,kron(P1,I9))) + kron(P2,kron(I9,kron(P2,I9)))
calc let EQU05 = kron(P0,kron(I27,kron(P0,I3))) + kron(P1,kron(I27,kron(P1,I3))) + kron(P2,kron(I27,kron(P2,I3)))
calc let EQU12 = kron(I3,kron(P0,kron(P0,I27))) + kron(I3,kron(P1,kron(P1,I27))) + kron(I3,kron(P2,kron(P2,I27)))
calc let EQU13 = kron(I3,kron(P0,kron(I3,kron(P0,I9)))) + kron(I3,kron(P1,kron(I3,kron(P1,I9)))) + kron(I3,kron(P2,kron(I3,kron(P2,I9))))
calc let EQU14 = kron(I3,kron(P0,kron(I9,kron(P0,I3)))) + kron(I3,kron(P1,kron(I9,kron(P1,I3)))) + kron(I3,kron(P2,kron(I9,kron(P2,I3))))
calc let EQU15 = kron(I3,kron(P0,kron(I27,P0))) + kron(I3,kron(P1,kron(I27,P1))) + kron(I3,kron(P2,kron(I27,P2)))
calc let EQU24 = kron(I9,kron(P0,kron(I3,kron(P0,I3)))) + kron(I9,kron(P1,kron(I3,kron(P1,I3)))) + kron(I9,kron(P2,kron(I3,kron(P2,I3))))
calc let EQU25 = kron(I9,kron(P0,kron(I9,P0))) + kron(I9,kron(P1,kron(I9,P1))) + kron(I9,kron(P2,kron(I9,P2)))
calc let EQU45 = kron(I81,kron(P0,P0)) + kron(I81,kron(P1,P1)) + kron(I81,kron(P2,P2))
calc let X3 = [0,0,1;1,0,0;0,1,0]
calc let G3 = X3 + dagger(X3)'

# ---------- БЛОК 1+2: проекторы, следы, миксер ----------
{
echo "$COMMON"
for e in 01 02 04 05 12 13 14 15 24 25 45; do echo "calc trace(EQU$e)"; done
echo 'calc let M1 = expm(-0.9*i*G3)'
echo 'calc trace(dagger(M1)*M1)'
echo 'calc det(M1)'
} > "$SESSION"
poler-engine --shell < "$SESSION" > /tmp/qutrit_qaoa_run1.txt 2>&1

echo "── БЛОК 1-2: проекторы равенства EQU_uv (след=243) и миксер ──" >> "$OUT"
python3 - << 'PYEOF' >> "$OUT"
vals, errors = [], []
for ln in open('/tmp/qutrit_qaoa_run1.txt', errors='replace'):
    if ln.startswith('poler> '):
        b = ln[7:].strip()
        if b.startswith('❌'): errors.append(b[:90]); continue
        try: vals.append(float(b.split(' ')[0].rstrip('i')))
        except ValueError: pass
print(f"чисел: {len(vals)} (ожид. 13: 11 следов + trace/det миксера), ошибок: {len(errors)}")
for e in errors[:3]: print(' ', e)
print('следы EQU (все = 243):', vals[:11])
if len(vals) > 12:
    print(f"миксер 3x3: trace(M†M)={vals[11]} (унитарность), det(M)={vals[12]} (=1, G3 бесследов)")
PYEOF

# ---------- БЛОК 3: брутфорс 729 назначений (отдельная арифметическая сессия) ----------
python3 - << 'PYEOF' > /tmp/qutrit_brute.session
edges = [(0,1,735),(0,2,24),(0,4,1496),(0,5,1222),(1,2,251),(1,3,231),(1,4,206),(1,5,194),(2,4,40),(2,5,28),(4,5,8)]
for x in range(729):
    c = [ (x // 3**j) % 3 for j in range(6) ]
    terms = [f"{w}*ceil(abs({c[u]}-{c[v]})/2)" for (u,v,w) in edges]
    print("calc " + " + ".join(terms))
PYEOF
poler-engine --shell < /tmp/qutrit_brute.session > /tmp/qutrit_brute_out.txt 2>&1
echo "" >> "$OUT"
echo "── БЛОК 3: брутфорс Max-3-Cut, 729 назначений (Калькулятор движка) ──" >> "$OUT"
python3 - << 'PYEOF' >> "$OUT"
vals, errors = [], []
for ln in open('/tmp/qutrit_brute_out.txt', errors='replace'):
    if ln.startswith('poler> '):
        b = ln[7:].strip()
        if b.startswith('❌'): errors.append(b[:90]); continue
        try: vals.append(float(b.split(' ')[0]))
        except ValueError: pass
cuts = dict(enumerate(vals))
print(f"просчитано {len(cuts)}/729, ошибок {len(errors)}")
if cuts:
    best = max(cuts.values())
    tops = sorted(cuts.items(), key=lambda kv: -kv[1])[:6]
    n_opt = sum(1 for v in cuts.values() if v == best)
    print(f"Max-3-Cut оптимум: {best:.0f} из 4435 ({100*best/4435:.1f}% всех рёбер) — арифметика движка")
    print(f"оптимальных назначений: {n_opt} (из 729, с учётом перестановок цветов)")
    print("топ-разбиения, x = c0+3c1+9c2+27c3+81c4+243c5:")
    for x, v in tops:
        c = [(x // 3**j) % 3 for j in range(6)]
        gangs = {0: [], 1: [], 2: []}
        for j, cj in enumerate(c):
            gangs[cj].append(j)
        print(f"  x={x}: cut={v:.0f} | A={gangs[0]} B={gangs[1]} C={gangs[2]}")
PYEOF

# ---------- БЛОК 4: QAOA-скан (gamma, beta) ----------
echo "" >> "$OUT"
echo "── БЛОК 4: QAOA-скан p=1, сетка 8x6=48 точек ──" >> "$OUT"
SCAN=/home/z/my-project/scripts/qutrit_qaoa_scan.session
if [ ! -s /tmp/qutrit_qaoa_scan.txt ] || grep -q "❌" /tmp/qutrit_qaoa_scan.txt; then
{
echo "$COMMON"
python3 - << 'PYEOF'
import itertools
edges = [(0,1,735/1496),(0,2,24/1496),(0,4,1.0),(0,5,1222/1496),(1,2,251/1496),(1,3,231/1496),(1,4,206/1496),(1,5,194/1496),(2,4,40/1496),(2,5,28/1496),(4,5,8/1496)]
names = ["01","02","04","05","12","13","14","15","24","25","45"]
for g, b in itertools.product([0.3,0.6,0.9,1.2,1.5,1.8,2.4,3.0],[0.3,0.6,0.9,1.2,1.6,2.0]):
    expr = "flat"
    for (u,v,w), n in zip(edges, names):
        expr = f"exp({g}*{w:.6f}*i)*({expr}) + (1 - exp({g}*{w:.6f}*i))*(EQU{n}*({expr}))"
    expr = f"kron(expm(-{b}*i*G3), kron(expm(-{b}*i*G3), kron(expm(-{b}*i*G3), kron(expm(-{b}*i*G3), kron(expm(-{b}*i*G3), expm(-{b}*i*G3))))))*({expr})"
    ec = " + ".join(f"{w:.6f}*(1 - trace(dagger(S)*EQU{n}*S))" for ((u,v,w),n) in zip(edges,names))
    print(f"calc let S = {expr}")
    print(f"calc {ec}")
PYEOF
} > "$SCAN"
poler-engine --shell < "$SCAN" > /tmp/qutrit_qaoa_scan.txt 2>&1
fi
python3 - << 'PYEOF' >> "$OUT"
import itertools
vals, errors = [], []
for ln in open('/tmp/qutrit_qaoa_scan.txt', errors='replace'):
    if ln.startswith('poler> '):
        b = ln[7:].strip()
        if b.startswith('❌'): errors.append(b[:90]); continue
        try: vals.append(float(b.split(' ')[0].rstrip('i')))
        except ValueError: pass
scan_vals = vals[-48:] if len(vals) >= 48 else []
print(f"значений скана: {len(scan_vals)}/48 (всего чисел {len(vals)}, ошибок {len(errors)})")
for e in errors[:2]: print(' ', e)
if len(scan_vals) == 48:
    grid = list(itertools.product([0.3,0.6,0.9,1.2,1.5,1.8,2.4,3.0],[0.3,0.6,0.9,1.2,1.6,2.0]))
    pairs = sorted(zip(grid, scan_vals), key=lambda kv: -kv[1])
    print(f"E[cut'] max = {pairs[0][1]:.4f} (норм.) = {pairs[0][1]*1496:.0f} синапсов; случайный = {4435/3:.0f}")
    print("топ-5 углов:")
    for (g,b),v in pairs[:5]:
        print(f"  gamma={g} beta={b}: E[cut]={v*1496:.0f}/4435")
PYEOF

# ---------- БЛОК 5: Born-концентрация на оптимуме ----------
BEST=$(python3 - << 'PYEOF'
import itertools
vals = []
for ln in open('/tmp/qutrit_qaoa_scan.txt', errors='replace'):
    if ln.startswith('poler> '):
        b = ln[7:].strip()
        if b.startswith('❌'): continue
        try: vals.append(float(b.split(' ')[0].rstrip('i')))
        except ValueError: pass
scan_vals = vals[-48:]
grid = list(itertools.product([0.3,0.6,0.9,1.2,1.5,1.8,2.4,3.0],[0.3,0.6,0.9,1.2,1.6,2.0]))
if len(scan_vals) == 48:
    (g,b),v = max(zip(grid, scan_vals), key=lambda kv: kv[1])
    print(f"{g} {b}")
else:
    print("1.2 0.9")
PYEOF
)
BG=$(echo "$BEST" | awk '{print $1}'); BB=$(echo "$BEST" | awk '{print $2}')
echo "" >> "$OUT"
echo "── БЛОК 5: Born-концентрация лучшего состояния (gamma=$BG, beta=$BB) ──" >> "$OUT"
{
echo "$COMMON"
python3 - "$BG" "$BB" << 'PYEOF'
import sys
g, b = float(sys.argv[1]), float(sys.argv[2])
edges = [(0,1,735/1496),(0,2,24/1496),(0,4,1.0),(0,5,1222/1496),(1,2,251/1496),(1,3,231/1496),(1,4,206/1496),(1,5,194/1496),(2,4,40/1496),(2,5,28/1496),(4,5,8/1496)]
names = ["01","02","04","05","12","13","14","15","24","25","45"]
expr = "flat"
for (u,v,w), n in zip(edges, names):
    expr = f"exp({g}*{w:.6f}*i)*({expr}) + (1 - exp({g}*{w:.6f}*i))*(EQU{n}*({expr}))"
expr = f"kron(expm(-{b}*i*G3), kron(expm(-{b}*i*G3), kron(expm(-{b}*i*G3), kron(expm(-{b}*i*G3), kron(expm(-{b}*i*G3), expm(-{b}*i*G3))))))*({expr})"
print(f"calc let S = {expr}")
raw = [(0,1,735),(0,2,24),(0,4,1496),(0,5,1222),(1,2,251),(1,3,231),(1,4,206),(1,5,194),(2,4,40),(2,5,28),(4,5,8)]
cuts = {}
for x in range(729):
    c = [(x // 3**j) % 3 for j in range(6)]
    cuts[x] = sum(w for (u,v,w) in raw if c[u] != c[v])
top3 = sorted(cuts.items(), key=lambda kv: -kv[1])[:3]
for rank,(x,cut) in enumerate(top3):
    c = [(x // 3**j) % 3 for j in range(6)]
    vecs = [f"[{1 if cj==0 else 0};{1 if cj==1 else 0};{1 if cj==2 else 0}]" for cj in c]
    sel = vecs[0]
    for vv in vecs[1:]:
        sel = f"kron({sel}, {vv})"
    print(f"calc let SEL{rank} = {sel}")
    print(f"calc abs(trace(dagger(SEL{rank})*S))^2")
PYEOF
} > /tmp/qutrit_born.session
poler-engine --shell < /tmp/qutrit_born.session > /tmp/qutrit_born.txt 2>&1
python3 - << 'PYEOF' >> "$OUT"
raw = [(0,1,735),(0,2,24),(0,4,1496),(0,5,1222),(1,2,251),(1,3,231),(1,4,206),(1,5,194),(2,4,40),(2,5,28),(4,5,8)]
cuts = {}
for x in range(729):
    c = [(x // 3**j) % 3 for j in range(6)]
    cuts[x] = sum(w for (u,v,w) in raw if c[u] != c[v])
top3 = sorted(cuts.items(), key=lambda kv: -kv[1])[:3]
vals, errors = [], []
for ln in open('/tmp/qutrit_born.txt', errors='replace'):
    if ln.startswith('poler> '):
        b = ln[7:].strip()
        if b.startswith('❌'): errors.append(b[:90]); continue
        try: vals.append(float(b.split(' ')[0].rstrip('i')))
        except ValueError: pass
print(f"перекрытий: {len(vals)}, ошибок {len(errors)}")
for e in errors[:2]: print(' ', e)
names = ['0=хаб','1=лейтенант','2=окт','3=ГАМК-a','4=АХ-a','5=АХ-b']
for (x, cut), p in zip(top3, vals[-3:]):
    c = [(x // 3**j) % 3 for j in range(6)]
    gangs = {0: [], 1: [], 2: []}
    for j, cj in enumerate(c):
        gangs[cj].append(names[j])
    print(f"x={x} (cut={cut}): Born P = {p:.4f} | {gangs}")
PYEOF

echo "" >> "$OUT"
{
echo "── ИТОГ Б: кутритный QAOA на живом коннектоме ──"
echo "Грабли: eye(n)<=64; E~константа e; kron строго 2 аргумента (вложенность);"
echo "тройки сайт-вложений легко потерять измерение (EQU24 давал 243 вместо 729);"
echo "разбор вывода — позиционный (грамматика не принимает метки)."
} >> "$OUT"
echo "готово: $OUT"
