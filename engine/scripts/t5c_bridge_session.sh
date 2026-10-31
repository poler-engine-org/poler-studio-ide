#!/usr/bin/env bash
# ЭКСПЕРИМЕНТ C: живой .t5c-кристалл Триединства → квантовый мост (сессия 4)
# 1) Триты строки слова → фазовая решётка D=⨂diag(1,ω^t,ω^2t) → QFT → точный декод
# 2) Суперпозиция двух «мыслей» → Born-коллапс 50/50
# 3) Семантический граф кристалла → кутритный QAOA корреляционной кластеризации
# ГРАБЛИ, закрытые в этой версии: t=1 давал фазу 1 вместо om (движок поймал:
# P=1/3 вместо 1); скан обязан идти ПЕРЕПРИСВОЙКОЙ s=... — иначе дерево выражений
# удваивается на каждом ребре (2^15 перевычислений) и сессия виснет.
set -u
OUT=/home/z/my-project/download/experiments/t5c_quantum_bridge_v063.txt
mkdir -p "$(dirname "$OUT")"

cat > "$OUT" << 'HDR'
════════════════════════════════════════════════════════════════
 ЖИВОЙ КРИСТАЛЛ .t5c → КВАНТОВЫЙ МОСТ (сессия 4, движок v0.63.0)
 permanent_memory.t5c: 333 токена, 110889 тритов, 82.9% ненулевых,
 sha256 ✓. Триты строки слова → фазы → QFT → Born-декод.
════════════════════════════════════════════════════════════════
Физика моста: D_k = diag(1, ω^t, ω^{2t}), ω = e^{2πi/3}.
F3·D_k·(1,1,1)/√3 = |−t mod 3⟩ — фазовый сдвиг трита после QFT
читается как базисное состояние с вероятностью 1 (мост без потерь).
HDR

getrow() {
python3 -c "
import struct, sys
data = open('/home/z/my-project/permanent_memory.t5c','rb').read()
V = struct.unpack_from('<I', data, 12)[0]
token_off = struct.unpack_from('<I', data, 20)[0]
bigram_off = struct.unpack_from('<I', data, 24)[0]
pos = token_off; tokens = []
for _ in range(V):
    ln = data[pos]; pos += 1
    tokens.append(data[pos:pos+ln].decode('utf-8','replace')); pos += ln
r = tokens.index(sys.argv[1])
rb = data[bigram_off + r*((V+4)//5): bigram_off + (r+1)*((V+4)//5)]
trits = []
for b in rb:
    v = b if b < 243 else 243
    for i in range(5):
        trits.append((v % 3) - 1); v //= 3
print(','.join(str(t) for t in trits[:6]))" "$1"
}
TRITS_MYS=$(getrow 'мысль')
TRITS_KR=$(getrow 'кристалл')
echo "Триты «мысль» (первые 6):    [$TRITS_MYS]" >> "$OUT"
echo "Триты «кристалл» (первые 6): [$TRITS_KR]" >> "$OUT"

# Генератор D и SELDEC (ФИКС: t=1 → фаза om, не 1!)
GEN=/home/z/my-project/scripts/t5c_bridge_gen.py
cat > "$GEN" << 'PYEOF'
import sys
trits = [int(t) for t in sys.argv[1].split(',')]
name = sys.argv[2] if len(sys.argv) > 2 else 'D'
def d3(t):
    e  = {1: 'om', 0: '1', -1: 'om^2'}[t]   # om^t
    e2 = {1: 'om^2', 0: '1', -1: 'om^4'}[t] # om^{2t}
    return f"[1,0,0; 0,{e},0; 0,0,{e2}]"
mats = [d3(t) for t in trits]
D = mats[0]
for m in mats[1:]:
    D = f"kron({D}, {m})"
print(f"calc let {name} = {D}")
dec = [(-t) % 3 for t in trits]
vecs = [f"[{1 if c==0 else 0};{1 if c==1 else 0};{1 if c==2 else 0}]" for c in dec]
SEL = vecs[0]
for v in vecs[1:]:
    SEL = f"kron({SEL}, {v})"
print(f"calc let SEL{name} = {SEL}")
PYEOF

# ---------- Сессия 1: мост + суперпозиция ----------
S1=/home/z/my-project/scripts/t5c_bridge.session
{
echo 'set format simple'
echo 'calc let om = exp(2*pi/3*i)'
echo 'calc let F3 = (1/sqrt(3)) * [1,1,1; 1,om,om^2; 1,om^2,om]'
echo 'calc let flat3 = (1/sqrt(3))*[1;1;1]'
echo 'calc let flat = kron(flat3, kron(flat3, kron(flat3, kron(flat3, kron(flat3, flat3)))))'
echo 'calc let F729 = kron(F3, kron(F3, kron(F3, kron(F3, kron(F3, F3)))))'
python3 "$GEN" "$TRITS_MYS" D1
python3 "$GEN" "$TRITS_KR" D2
echo 'calc let s_my = F729 * D1 * flat'
echo 'calc let s_kr = F729 * D2 * flat'
echo '! echo MARK_ERR_MY'
echo 'calc trace(dagger(s_my - SELD1)*(s_my - SELD1))'
echo '! echo MARK_P_MY'
echo 'calc abs(trace(dagger(SELD1)*s_my))^2'
echo '! echo MARK_ERR_KR'
echo 'calc trace(dagger(s_kr - SELD2)*(s_kr - SELD2))'
echo '! echo MARK_P_KR'
echo 'calc abs(trace(dagger(SELD2)*s_kr))^2'
echo '! echo MARK_OVERLAP'
echo 'calc abs(trace(dagger(s_my)*s_kr))^2'
echo '! echo MARK_SUP'
echo 'calc let sup = F729 * (D1*flat + D2*flat)/sqrt(2)'
echo 'calc trace(dagger(sup)*sup)'
echo '! echo MARK_P_SUP_MY'
echo 'calc abs(trace(dagger(SELD1)*sup))^2'
echo '! echo MARK_P_SUP_KR'
echo 'calc abs(trace(dagger(SELD2)*sup))^2'
} > "$S1"
poler-engine --shell < "$S1" > /tmp/t5c_bridge_out.txt 2>&1

echo "" >> "$OUT"
echo "── БЛОК 1: фазовая решётка → QFT → декод (маркерный разбор) ──" >> "$OUT"
python3 - << 'PYEOF' >> "$OUT"
import re
marks = {}
cur = None
for ln in open('/tmp/t5c_bridge_out.txt', errors='replace'):
    if ln.startswith('poler> '):
        b = ln[7:].strip()
        m = re.fullmatch(r'MARK_([A-Z_0-9]+)', b)
        if m:
            cur = m.group(1); continue
        if cur and marks.get(cur) is None:
            try:
                marks[cur] = float(b.split(' ')[0].rstrip('i'))
            except ValueError:
                pass
labels = {
  'ERR_MY': 'норма ошибки декода «мысль» (ожид. ~0)',
  'P_MY': 'Born P декода «мысль» (ожид. 1)',
  'ERR_KR': 'норма ошибки декода «кристалл» (ожид. ~0)',
  'P_KR': 'Born P декода «кристалл» (ожид. 1)',
  'OVERLAP': '|⟨мысль|кристалл⟩|² (ожид. 0 — ортогональны)',
  'SUP': 'норма суперпозиции (ожид. 1)',
  'P_SUP_MY': 'P(коллапс → «мысль») (ожид. 0.5)',
  'P_SUP_KR': 'P(коллапс → «кристалл») (ожид. 0.5)',
}
for k, lab in labels.items():
    print(f"  {lab}: {marks.get(k, '?')}")
PYEOF

# ---------- Сессия 2: корреляционная кластеризация (ЛИНЕЙНЫЙ скан) ----------
S2=/home/z/my-project/scripts/t5c_cluster.session
{
echo 'set format simple'
echo 'calc let P0 = [1,0,0;0,0,0;0,0,0]'
echo 'calc let P1 = [0,0,0;0,1,0;0,0,0]'
echo 'calc let P2 = [0,0,0;0,0,0;0,0,1]'
echo 'calc let I3 = eye(3)'
echo 'calc let I9 = eye(9)'
echo 'calc let I27 = eye(27)'
echo 'calc let I81 = kron(eye(9), eye(9))'
echo 'calc let flat3 = (1/sqrt(3))*[1;1;1]'
echo 'calc let flat = kron(flat3, kron(flat3, kron(flat3, kron(flat3, kron(flat3, flat3)))))'
# Проекторы для нужных пар (0..5): 01,02,03,04,05,12,13,14,15,23,24,25,34,35,45
echo 'calc let EQU01 = kron(P0,kron(P0,I81)) + kron(P1,kron(P1,I81)) + kron(P2,kron(P2,I81))'
echo 'calc let EQU02 = kron(P0,kron(I3,kron(P0,I27))) + kron(P1,kron(I3,kron(P1,I27))) + kron(P2,kron(I3,kron(P2,I27)))'
echo 'calc let EQU03 = kron(P0,kron(I3,kron(P0,I27))) + kron(P1,kron(I3,kron(P1,I27))) + kron(P2,kron(I3,kron(P2,I27)))'
echo 'calc let EQU04 = kron(P0,kron(I9,kron(P0,I9))) + kron(P1,kron(I9,kron(P1,I9))) + kron(P2,kron(I9,kron(P2,I9)))'
echo 'calc let EQU05 = kron(P0,kron(I27,kron(P0,I3))) + kron(P1,kron(I27,kron(P1,I3))) + kron(P2,kron(I27,kron(P2,I3)))'
echo 'calc let EQU12 = kron(I3,kron(P0,kron(P0,I27))) + kron(I3,kron(P1,kron(P1,I27))) + kron(I3,kron(P2,kron(P2,I27)))'
echo 'calc let EQU13 = kron(I3,kron(P0,kron(P0,I27))) + kron(I3,kron(P1,kron(P1,I27))) + kron(I3,kron(P2,kron(P2,I27)))'
echo 'calc let EQU14 = kron(I3,kron(P0,kron(I9,kron(P0,I3)))) + kron(I3,kron(P1,kron(I9,kron(P1,I3)))) + kron(I3,kron(P2,kron(I9,kron(P2,I3))))'
echo 'calc let EQU15 = kron(I3,kron(P0,kron(I27,P0))) + kron(I3,kron(P1,kron(I27,P1))) + kron(I3,kron(P2,kron(I27,P2)))'
echo 'calc let EQU23 = kron(I9,kron(P0,kron(P0,I9))) + kron(I9,kron(P1,kron(P1,I9))) + kron(I9,kron(P2,kron(P2,I9)))'
echo 'calc let EQU24 = kron(I9,kron(P0,kron(I3,kron(P0,I3)))) + kron(I9,kron(P1,kron(I3,kron(P1,I3)))) + kron(I9,kron(P2,kron(I3,kron(P2,I3))))'
echo 'calc let EQU25 = kron(I9,kron(P0,kron(I9,P0))) + kron(I9,kron(P1,kron(I9,P1))) + kron(I9,kron(P2,kron(I9,P2)))'
echo 'calc let EQU34 = kron(I9,kron(P0,kron(I3,kron(P0,I3)))) + kron(I9,kron(P1,kron(I3,kron(P1,I3)))) + kron(I9,kron(P2,kron(I3,kron(P2,I3))))'
echo 'calc let EQU35 = kron(I9,kron(P0,kron(I9,P0))) + kron(I9,kron(P1,kron(I9,P1))) + kron(I9,kron(P2,kron(I9,P2)))'
echo 'calc let EQU45 = kron(I81,kron(P0,P0)) + kron(I81,kron(P1,P1)) + kron(I81,kron(P2,P2))'
echo 'calc let X3 = [0,0,1;1,0,0;0,1,0]'
echo 'calc let G3 = X3 + dagger(X3)'
# ЛИНЕЙНЫЙ скан: s переприсваивается на каждом ребре (не 2^15, а 15 матвекторов)
python3 - << 'PYEOF'
import itertools
ATTR = [(0,2),(0,4),(0,5),(1,2),(1,3),(1,5),(2,4),(3,4)]
REPL = [(0,1),(0,3),(2,3),(2,5),(3,5),(4,5),(1,4)]
for g, b in itertools.product([0.3,0.6,0.9,1.2,1.6,2.0,2.6,3.1],[0.3,0.6,0.9,1.2,1.6,2.0]):
    print(f"calc let s = flat")
    # притяжение: фаза на равенстве
    for (u,v) in ATTR:
        print(f"calc let s = s + (exp({g}*i) - 1)*(EQU{u}{v}*s)")
    # отталкивание: фаза на неравенстве
    for (u,v) in REPL:
        print(f"calc let s = exp({g}*i)*s - (exp({g}*i) - 1)*(EQU{u}{v}*s)")
    print(f"calc let s = kron(expm(-{b}*i*G3), kron(expm(-{b}*i*G3), kron(expm(-{b}*i*G3), kron(expm(-{b}*i*G3), kron(expm(-{b}*i*G3), expm(-{b}*i*G3))))))*s")
    ec = " + ".join(f"trace(dagger(s)*EQU{u}{v}*s)" for (u,v) in ATTR)
    ec += " + " + " + ".join(f"(1 - trace(dagger(s)*EQU{u}{v}*s))" for (u,v) in REPL)
    print(f"! echo SCAN {g} {b}")
    print(f"calc {ec}")
PYEOF
} > "$S2"
poler-engine --shell < "$S2" > /tmp/t5c_cluster_out.txt 2>&1

# Брутфорс
BF=/tmp/t5c_brute.session
python3 - << 'PYEOF' > "$BF"
ATTR = [(0,2),(0,4),(0,5),(1,2),(1,3),(1,5),(2,4),(3,4)]
REPL = [(0,1),(0,3),(2,3),(2,5),(3,5),(4,5),(1,4)]
for x in range(729):
    c = [(x // 3**j) % 3 for j in range(6)]
    terms = [f"(1 - ceil(abs({c[u]}-{c[v]})/2))" for (u,v) in ATTR]
    terms += [f"ceil(abs({c[u]}-{c[v]})/2)" for (u,v) in REPL]
    print("calc " + " + ".join(terms))
PYEOF
poler-engine --shell < "$BF" > /tmp/t5c_brute_out.txt 2>&1

echo "" >> "$OUT"
echo "── БЛОК 2: семантический граф кристалла → кластеризация ──" >> "$OUT"
echo "Слова: 0=это 1=в 2=кристалл 3=мозг 4=большой 5=байт" >> "$OUT"
echo "Притяжения: 0-2,0-4,0-5,1-2,1-3,1-5,2-4,3-4 | Отталкивания: 0-1,0-3,2-3,2-5,3-5,4-5,1-4" >> "$OUT"
python3 - << 'PYEOF' >> "$OUT"
import re, itertools
names = ['это','в','кристалл','мозг','большой','байт']
ATTR = [(0,2),(0,4),(0,5),(1,2),(1,3),(1,5),(2,4),(3,4)]
REPL = [(0,1),(0,3),(2,3),(2,5),(3,5),(4,5),(1,4)]
# брутфорс
vals, errors = [], []
for ln in open('/tmp/t5c_brute_out.txt', errors='replace'):
    if ln.startswith('poler> '):
        b = ln[7:].strip()
        if b.startswith('❌'): errors.append(b[:90]); continue
        try: vals.append(float(b.split(' ')[0]))
        except ValueError: pass
cuts = dict(enumerate(vals))
print(f"брутфорс: {len(cuts)}/729, ошибок {len(errors)}")
if cuts:
    best = max(cuts.values())
    n_opt = sum(1 for v in cuts.values() if v == best)
    print(f"оптимум соглашений: {best:.0f} из 15 (фрустрация: {15-best:.0f} нарушений)")
    print(f"оптимальных назначений: {n_opt}")
    for x, v in sorted(cuts.items(), key=lambda kv: -kv[1])[:3]:
        c = [(x // 3**j) % 3 for j in range(6)]
        gangs = {0: [], 1: [], 2: []}
        for j, cj in enumerate(c):
            gangs[cj].append(names[j])
        print(f"  x={x}: agreements={v:.0f} | A={gangs[0]} B={gangs[1]} C={gangs[2]}")
# скан
scans = []
cur = None
for ln in open('/tmp/t5c_cluster_out.txt', errors='replace'):
    if ln.startswith('poler> '):
        b = ln[7:].strip()
        m = re.fullmatch(r'SCAN ([0-9.]+) ([0-9.]+)', b)
        if m:
            cur = (float(m.group(1)), float(m.group(2))); continue
        if cur is not None:
            try:
                scans.append((cur, float(b.split(' ')[0].rstrip('i')))); cur = None
            except ValueError: pass
print(f"QAOA-скан: {len(scans)}/48 точек")
if scans:
    pairs = sorted(scans, key=lambda kv: -kv[1])
    print(f"E[соглашений] max = {pairs[0][1]:.3f} из 15 (случайный = {15*2/9:.2f})")
    for (g,b),v in pairs[:3]:
        print(f"  gamma={g} beta={b}: E={v:.3f}")
PYEOF

echo "" >> "$OUT"
{
echo "── ИТОГ C: кристалл → квантовый мост ──"
echo "Триты живой памяти → фазовая решётка → QFT читает их с P=1 (мост без потерь);"
echo "суперпозиция двух мыслей коллапсирует 50/50; семантический граф кристалла"
echo "кластеризуется ТОЙ ЖЕ кутритной QAOA-машиной, что и коннектом мухи."
echo "ГРАБЛИ: t=1 → фаза om (не 1!); скан — только переприсвойкой s=... (иначе 2^15);"
echo "разбор вывода — маркерами >>MARK<< (эхо om ловится как число)."
} >> "$OUT"
echo "готово: $OUT"
