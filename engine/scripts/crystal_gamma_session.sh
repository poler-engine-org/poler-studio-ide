#!/usr/bin/env bash
# СЕССИЯ 7 — «ПАМЯТЬ ВЫБИРАЕТ НЕКОМПАКТНОСТЬ γ» (пользователь: «прокормить
# фазы Ацина тритами кристалла — пусть некомпактность γ выбирает сама память»).
#
# Источник: permanent_memory.t5c (8198×8198 тритов, 67 182 610 6-тритовых
# окон просканировано, scripts/session7/crystal_gamma_scan.py). Память
# выбирает argmax I₃ по собственным окнам:
#   окно [1, 0, −1, 0, −1, 1] (13 вхождений; строки 'and','to','in',...)
#     → γ_mem = 578/729 = 0.7928669410
#     → I₃(γ_mem) = 2.9148538425 = 99.999987% оптимума Ацина
#   (γ* = (√11−√3)/2 = 0.7922870 недостижим из 6 тритов: ближайшее 578/729)
#
# Аналитика семейства Ацина (движок, сессия 7): I₃(γ) = 4(2√3γ+3)/(3(2+γ²)).
#
# Верификация на Калькуляторе:
#   A) контур CGLMP I₃(γ_mem) == аналитике 4(2√3γ+3)/(3(2+γ²))
#   B) I₃(γ_mem) < I₃(γ*) = 1+√(11/3), но > Белла 2.872934 — память почти
#      на пределе, алгебра чуть точнее
#   C) МОСТ: те же триты окна → фазовая решётка D → QFT → |−t mod 3⟩ с P=1
#      («фазы накормлены тритами» буквально)
set -u
ENG="${ENG:-poler-engine}"
OUT="${OUT:-/home/z/my-project/download/experiments/crystal_gamma_v063.txt}"
SESS="scripts/crystal_gamma.session"
GEN="scripts/t5c_bridge_gen.py"
mkdir -p "$(dirname "$OUT")"
exec > >(tee "$OUT") 2>&1

echo "═══ ПАМЯТЬ ВЫБИРАЕТ γ АЦИНА — $(date -u '+%Y-%m-%d %H:%M UTC') ═══"
echo "Движок: $("$ENG" --exec 'version' < /dev/null 2>&1 | head -1)"
echo "Кристалл: 8198×8198 тритов, 67 182 610 окон, выбор n*=578 ([1,0,-1,0,-1,1])"

{
echo 'set format simple'
echo 'calc let om = exp(2*pi/3*i)'
echo 'calc let F3 = (1/sqrt(3)) * [1,1,1; 1,om,om^2; 1,om^2,om]'
echo 'calc let e0 = [1; 0; 0]'
echo 'calc let e1 = [0; 1; 0]'
echo 'calc let e2 = [0; 0; 1]'
echo 'calc let A2p = [1,0,0; 0,exp(pi/3*i),0; 0,0,exp(2*pi/3*i)]'
echo 'calc let B1p = [1,0,0; 0,exp(pi/6*i),0; 0,0,exp(pi/3*i)]'
echo 'calc let B2p = [1,0,0; 0,exp(-pi/6*i),0; 0,0,exp(-pi/3*i)]'
echo 'calc let FF = kron(F3, dagger(F3))'
echo 'calc let Pc0 = kron(e0,e0)*transpose(kron(e0,e0)) + kron(e1,e1)*transpose(kron(e1,e1)) + kron(e2,e2)*transpose(kron(e2,e2))'
echo 'calc let Pcp = kron(e1,e0)*transpose(kron(e1,e0)) + kron(e2,e1)*transpose(kron(e2,e1)) + kron(e0,e2)*transpose(kron(e0,e2))'
echo 'calc let Pcm = kron(e2,e0)*transpose(kron(e2,e0)) + kron(e0,e1)*transpose(kron(e0,e1)) + kron(e1,e2)*transpose(kron(e1,e2))'
echo 'calc let M11 = kron(eye(3), B1p)'
echo 'calc let M12 = kron(eye(3), B2p)'
echo 'calc let M21 = kron(A2p, B1p)'
echo 'calc let M22 = kron(A2p, B2p)'
# ВЫБОР ПАМЯТИ
echo 'calc let gmem = 578/729'
echo 'calc let nnm = 2 + gmem^2'
echo 'calc let mvm = (1/sqrt(nnm)) * [1;0;0;0;gmem;0;0;0;1]'
echo 'calc let s11 = FF * M11 * mvm'
echo 'calc let s12 = FF * M12 * mvm'
echo 'calc let s21 = FF * M21 * mvm'
echo 'calc let s22 = FF * M22 * mvm'
echo '! echo MARK_GMEM'
echo 'calc gmem'
echo '! echo MARK_I3_MEM'
echo 'calc trace(dagger(s11)*Pc0*s11) + trace(dagger(s21)*Pcm*s21) + trace(dagger(s22)*Pc0*s22) + trace(dagger(s12)*Pc0*s12) - trace(dagger(s11)*Pcm*s11) - trace(dagger(s21)*Pc0*s21) - trace(dagger(s22)*Pcm*s22) - trace(dagger(s12)*Pcp*s12)'
echo '! echo MARK_I3_MEM_LIT'
echo 'calc 4*(2*sqrt(3)*gmem + 3)/(3*(2 + gmem^2))'
echo '! echo MARK_I3_ACIN'
echo 'calc 1 + sqrt(11/3)'
echo '! echo MARK_I3_BELL'
echo 'calc (4/9)*(3+2*sqrt(3))'
# МОСТ: триты окна [1,0,-1,0,-1,1] -> D -> QFT -> |−t mod 3⟩
python3 "$GEN" "1,0,-1,0,-1,1" DMEM
echo 'calc let F729 = kron(F3, kron(F3, kron(F3, kron(F3, kron(F3, F3)))))'
echo 'calc let flat6 = kron((1/sqrt(3))*[1;1;1], kron((1/sqrt(3))*[1;1;1], kron((1/sqrt(3))*[1;1;1], kron((1/sqrt(3))*[1;1;1], kron((1/sqrt(3))*[1;1;1],(1/sqrt(3))*[1;1;1])))))'
echo 'calc let s_mem = F729 * DMEM * flat6'
echo '! echo MARK_BRIDGE_ERR'
echo 'calc trace(dagger(s_mem - SELDMEM)*(s_mem - SELDMEM))'
echo '! echo MARK_BRIDGE_P'
echo 'calc abs(trace(dagger(SELDMEM)*s_mem))^2'
# связка: γ из окна = 578/729 = цифры окна (2,1,0,1,0,2) как база-3 дробь
echo '! echo MARK_GAMMA_FROM_WINDOW'
echo 'calc (2/3 + 1/9 + 0/27 + 1/81 + 0/243 + 2/729)'
echo '! echo MARK_DONE'
} > "$SESS"

echo "── запуск сессии ($SESS, $(wc -l < "$SESS") строк)"
T0=$(date +%s)
timeout 300 "$ENG" --shell < "$SESS" > /tmp/crystal_gamma_out.txt 2>&1
RC=$?
T1=$(date +%s)
echo "exit=$RC, время: $((T1-T0)) с"

echo "── разбор маркеров"
python3 - << 'PYEOF'
import re
vals = {}
cur = None
for ln in open('/tmp/crystal_gamma_out.txt', errors='replace'):
    ln = ln.rstrip()
    if ln.startswith('poler> '):
        ln = ln[7:]
    if ln.startswith('MARK_'):
        cur = ln
        vals[cur] = []
    elif cur is not None:
        s = ln.strip()
        m = re.match(r'^-?\d+\.?\d*(e[+-]?\d+)?$', s)
        mc = re.match(r'^(-?\d+\.?\d*(?:e[+-]?\d+)?)\s*([+-])\s*(\d+\.?\d*(?:e[+-]?\d+)?)i$', s)
        if m:
            vals[cur].append(float(s))
        elif mc:
            re_, sg, im = float(mc.group(1)), mc.group(2), float(mc.group(3))
            if sg == '-':
                im = -im
            if abs(im) < 1e-12:
                vals[cur].append(re_)
            else:
                cur = None
        elif s == '':
            cur = None
        else:
            cur = None
def g(mark, i=0):
    v = vals.get(mark, [])
    return v[i] if len(v) > i else None
gm = g('MARK_GMEM')
print(f"γ_mem (движок)          = {gm}   (= 578/729 = 0.7928669410)")
i3m, i3l = g('MARK_I3_MEM'), g('MARK_I3_MEM_LIT')
print(f"I₃(γ_mem) контур        = {i3m}")
print(f"I₃ аналитика 4(2√3γ+3)/(3(2+γ²)) = {i3l}")
if i3m is not None and i3l is not None:
    print(f"  расхождение           = {abs(i3m-i3l):.2e}  {'✓ КОНТУР == АНАЛИТИКА' if abs(i3m-i3l)<1e-10 else '✗'}")
i3a = g('MARK_I3_ACIN')
i3b = g('MARK_I3_BELL')
print(f"I₃(γ*) Ацин (предел)    = {i3a}")
print(f"I₄(Белл) компакт        = {i3b}")
if i3m is not None and i3a is not None and i3b is not None:
    print(f"  память: {100*i3m/i3a:.6f}% оптимума Ацина; "
          f"выше Белла: {100*(i3m/i3b-1):.3f}%")
    print(f"  зазор до предела      = {i3a-i3m:.2e} — алгебра точнее тритов 6-го порядка")
berr, bp = g('MARK_BRIDGE_ERR'), g('MARK_BRIDGE_P')
print(f"МОСТ ‖F·D·flat − |−t⟩‖² = {berr}   {'✓' if berr is not None and berr < 1e-20 else '?'}")
print(f"МОСТ P(декод) = |⟨−t|F·D·flat⟩|² = {bp}   {'✓ ФАЗЫ НАКОРМЛЕНЫ ТРИТАМИ' if bp is not None and abs(bp-1) < 1e-10 else '?'}")
gw = g('MARK_GAMMA_FROM_WINDOW')
print(f"γ из цифр окна (2/3+1/9+1/81+2/729) = {gw}  {'✓ == 578/729' if gw is not None and abs(gw-578/729)<1e-12 else '?'}")
PYEOF
echo "═══ КОНЕЦ: ПАМЯТЬ ↔ АЦИН ═══"
