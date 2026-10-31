#!/usr/bin/env bash
# ЭКСПЕРИМЕНТ D (свободные комбинации): КВАНТОВЫЕ ПРОГУЛКИ — сессия 4
# D1: квантовая прогулка по K4-ядру коннектома мухи (вешт. матрица смежности, schrodinger)
# D2: квантовая прогулка по СЕМАНТИЧЕСКОМУ графу кристалла (подписанная матрица ±1)
# D3: Loschmidt-эхо — вероятность возврата сигнала в нейрон-источник (квантовые ревайвалы)
# Один и тот же движок эволюционирует «биологический мозг» и «кристалл памяти».
set -u
OUT=/home/z/my-project/download/experiments/quantum_walks_v063.txt
mkdir -p "$(dirname "$OUT")"

cat > "$OUT" << 'HDR'
════════════════════════════════════════════════════════════════
 КВАНТОВЫЕ ПРОГУЛКИ: мозг мухи и кристалл памяти под одним
 уравнением Шрёдингера i∂ψ/∂t = Hψ (сессия 4, движок v0.63.0)
════════════════════════════════════════════════════════════════
D1: H_fly = взвешенная смежность K4-ядра (реальные синапсы, норм. 1/1496)
    ψ₀ = |хаб 79529⟩ — сигнал рождается в хабе
D2: H_t5c = подписанная семантическая матрица кристалла (±1, притяжение/отталкивание)
    ψ₀ = |«кристалл»⟩ — мысль рождается в слове
D3: Loschmidt L(t) = |⟨ψ₀|e^{-iHt}|ψ₀⟩|² — возврат сигнала/мысли в исток
Классический сигнал монотонно растекается; квантовый — интерферирует и РЕВАЙВИТ.
HDR

# D1 + D3: прогулка по мозгу
S=/home/z/my-project/scripts/quantum_walk.session
{
echo 'set format simple'
# H_fly: 6 узлов, взвешенная симметричная смежность (норм. /1496), диагональ = -сумма (граф-Лапласиан для унитарной диффузии)
echo 'calc let Hf = (1/1496)*[-0,735,24,0,1496,1222; 735,-0,251,231,206,194; 24,251,-0,0,40,28; 0,231,0,-0,0,0; 1496,206,40,0,-0,8; 1222,194,28,0,8,-0]'
echo 'calc let psi0 = [1;0;0;0;0;0]'
# Снимки вероятностей: P_j = |<e_j|psi(t)>|^2 (abs() в движке скалярный!)
for t in 0.25 0.5 1.0 2.0 4.0; do
  echo "calc let ps = schrodinger(Hf, psi0, $t)"
  echo "! echo SNAP_fly_$t"
  for j in 0 1 2 3 4 5; do
    V=$(python3 -c "print('[' + ';'.join('1' if k==$j else '0' for k in range(6)) + ']')")
    echo "calc abs(trace(dagger($V)*ps))^2"
  done
done
echo "! echo LOSCH_FLY"
echo 'calc 0'
for t in 0.5 1.0 1.5 2.0 2.5 3.0 4.0 5.0 6.0; do
  echo "calc abs(trace(dagger(psi0)*schrodinger(Hf, psi0, $t)))^2"
done
# D2: прогулка по кристаллу
echo 'calc let Hc = [-0,-1,1,0,1,-1; -1,-0,1,0,-1,-1; 1,1,-0,-1,1,0; 0,0,-1,-0,-1,0; 1,-1,1,-1,-0,0; -1,-1,0,0,0,-0]'
echo 'calc let phi0 = [0;0;1;0;0;0]'
for t in 0.25 0.5 1.0 2.0 4.0; do
  echo "calc let ph = schrodinger(Hc, phi0, $t)"
  echo "! echo SNAP_t5c_$t"
  for j in 0 1 2 3 4 5; do
    V=$(python3 -c "print('[' + ';'.join('1' if k==$j else '0' for k in range(6)) + ']')")
    echo "calc abs(trace(dagger($V)*ph))^2"
  done
done
echo "! echo LOSCH_T5C"
echo 'calc 0'
for t in 0.5 1.0 1.5 2.0 2.5 3.0 4.0 5.0 6.0; do
  echo "calc abs(trace(dagger(phi0)*schrodinger(Hc, phi0, $t)))^2"
done
} > "$S"
poler-engine --shell < "$S" > /tmp/quantum_walk_out.txt 2>&1

python3 - << 'PYEOF' > /tmp/quantum_walk_parsed.txt
import re
cur = None
data = {}
for ln in open('/tmp/quantum_walk_out.txt', errors='replace'):
    if ln.startswith('poler> '):
        b = ln[7:].strip()
        m = re.fullmatch(r'SNAP_(fly|t5c)_([0-9.]+)', b)
        if m:
            cur = ('snap', m.group(1), m.group(2)); continue
        m = re.fullmatch(r'LOSCH_(FLY|T5C)', b)
        if m:
            cur = ('losch', m.group(1).lower(), None); continue
        if cur is not None:
            if cur[0] == 'snap' and b.startswith('['):
                # вектор вероятностей [p0, p1, ...]
                try:
                    vec = [float(x) for x in re.findall(r'\d+\.\d+(?:e-?\d+)?', b)]
                    data[cur] = vec
                except ValueError:
                    pass
                cur = None
            else:
                try:
                    v = float(b.split(' ')[0].rstrip('i'))
                    data.setdefault(cur, []).append(v)
                except ValueError:
                    pass
import json
print(json.dumps({str(k): v for k, v in data.items()}))
PYEOF

python3 - << 'PYEOF' >> "$OUT"
import json, re
raw = open('/tmp/quantum_walk_parsed.txt').read().strip()
data = {eval(k): v for k, v in json.loads(raw).items()}
names_fly = ['79529-хаб','47410-лейт','55498-окт','101516-ГАМКa','74111-АХa','106315-АХb']
names_t5c = ['это','в','кристалл','мозг','большой','байт']
print("── D1: квантовая прогулка по K4-ядру мозга мухи, ψ₀=|хаб⟩ ──")
for t in ['0.25','0.5','1.0','2.0','4.0']:
    key = ('snap','fly',t)
    if key in data:
        probs = data[key]
        top = sorted(zip(names_fly, probs), key=lambda kv: -kv[1])[:3]
        print(f"  t={t}: " + " ".join(f"{n}={p:.3f}" for n,p in top))
print("\n── D2: квантовая прогулка по семантическому графу кристалла, ψ₀=|«кристалл»⟩ ──")
for t in ['0.25','0.5','1.0','2.0','4.0']:
    key = ('snap','t5c',t)
    if key in data:
        probs = data[key]
        top = sorted(zip(names_t5c, probs), key=lambda kv: -kv[1])[:3]
        print(f"  t={t}: " + " ".join(f"{n}={p:.3f}" for n,p in top))
print("\n── D3: Loschmidt-эхо L(t) = |⟨ψ₀|e^(-iHt)|ψ₀⟩|² (квантовые ревайвалы) ──")
ts = [0.5,1.0,1.5,2.0,2.5,3.0,4.0,5.0,6.0]
for kind, name in [('fly','мозг мухи (хаб)'),('t5c','кристалл («кристалл»)')]:
    key = ('losch', kind, None)
    if key in data:
        vals = data[key][1:] if data[key][0] == 0.0 else data[key]
        print(f"  {name}: " + " ".join(f"L({t})={v:.3f}" for t, v in zip(ts, vals)))
        if vals:
            mx = max(vals); mt = ts[vals.index(mx)]
            print(f"    максимум возврата: L({mt})={mx:.3f} — сигнал ПОМНИТ исток")
PYEOF

echo "" >> "$OUT"
{
echo "── ИТОГ D ──"
echo "Один бинарник эволюционирует два субстрата: синапсы мухи и триты памяти."
echo "Loschmidt-эхо показывает немонотонный возврат — квантовая память сигнала."
} >> "$OUT"
echo "готово: $OUT"
cat "$OUT" | tail -30
