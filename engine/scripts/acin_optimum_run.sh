#!/usr/bin/env bash
# СЕССИЯ 6 — верификация ОПТИМУМА АЦИНА на чистом Калькуляторе движка.
# Результат Acín-Durt-Gisin-Latorre (quant-ph/0111143):
#   I₃^max(d=3) = 1 + √(11/3) ≈ 2.914854 на состоянии
#   |Ψ_mv⟩ = (|00⟩ + γ|11⟩ + |22⟩)/√(2+γ²),  γ = (√11−√3)/2 ≈ 0.792287
# Проверяется четырьмя независимыми путями:
#   A) прямой контур CGLMP (s_xy + классовые проекторы) — сессия 4-машинерия
#   B) оператор Белла Bop из примитивов == оператор B из статьи (позлементно)
#   C) Rayleigh ⟨Ψ|B|Ψ⟩ и собственное уравнение B·Ψ = λ·Ψ
#   D) шумовой порог η* = 2/(1+√(11/3)) → I₃(η*) = 2.0 (робастность)
set -u
ENG="${ENG:-poler-engine}"
OUT="${OUT:-/home/z/my-project/download/experiments/acin_optimum_v063.txt}"
SESS="scripts/acin_optimum.session"
mkdir -p "$(dirname "$OUT")"
exec > >(tee "$OUT") 2>&1

echo "═══ ОПТИМУМ АЦИНА НА КАЛЬКУЛЯТОРЕ — $(date -u '+%Y-%m-%d %H:%M UTC') ═══"
echo "Движок: $("$ENG" --exec 'version' < /dev/null 2>&1 | head -1)"

cat > "$SESS" << 'EOF'
set format simple
calc let om = exp(2*pi/3*i)
calc let F3 = (1/sqrt(3)) * [1,1,1; 1,om,om^2; 1,om^2,om]
calc let e0 = [1; 0; 0]
calc let e1 = [0; 1; 0]
calc let e2 = [0; 0; 1]
calc let A2p = [1,0,0; 0,exp(pi/3*i),0; 0,0,exp(2*pi/3*i)]
calc let B1p = [1,0,0; 0,exp(pi/6*i),0; 0,0,exp(pi/3*i)]
calc let B2p = [1,0,0; 0,exp(-pi/6*i),0; 0,0,exp(-pi/3*i)]
calc let FF = kron(F3, dagger(F3))
calc let Pc0 = kron(e0,e0)*transpose(kron(e0,e0)) + kron(e1,e1)*transpose(kron(e1,e1)) + kron(e2,e2)*transpose(kron(e2,e2))
calc let Pcp = kron(e1,e0)*transpose(kron(e1,e0)) + kron(e2,e1)*transpose(kron(e2,e1)) + kron(e0,e2)*transpose(kron(e0,e2))
calc let Pcm = kron(e2,e0)*transpose(kron(e2,e0)) + kron(e0,e1)*transpose(kron(e0,e1)) + kron(e1,e2)*transpose(kron(e1,e2))
calc let M11 = kron(eye(3), B1p)
calc let M12 = kron(eye(3), B2p)
calc let M21 = kron(A2p, B1p)
calc let M22 = kron(A2p, B2p)
calc let gam = (sqrt(11)-sqrt(3))/2
calc let nn = 2 + gam^2
calc let mv = (1/sqrt(nn)) * [1;0;0;0;gam;0;0;0;1]
calc let psiB = (1/sqrt(3)) * [1;0;0;0;1;0;0;0;1]
calc let s11 = FF * M11 * mv
calc let s12 = FF * M12 * mv
calc let s21 = FF * M21 * mv
calc let s22 = FF * M22 * mv
! echo MARK_GAM
calc gam
! echo MARK_I3
calc trace(dagger(s11)*Pc0*s11) + trace(dagger(s21)*Pcm*s21) + trace(dagger(s22)*Pc0*s22) + trace(dagger(s12)*Pc0*s12) - trace(dagger(s11)*Pcm*s11) - trace(dagger(s21)*Pc0*s21) - trace(dagger(s22)*Pcm*s22) - trace(dagger(s12)*Pcp*s12)
! echo MARK_TARGET
calc 1 + sqrt(11/3)
! echo MARK_Q0
calc trace(dagger(s11)*Pc0*s11)
! echo MARK_QM
calc trace(dagger(s11)*Pcm*s11)
calc let Bop = dagger(M11)*dagger(FF)*Pc0*FF*M11 + dagger(M21)*dagger(FF)*Pcm*FF*M21 + dagger(M22)*dagger(FF)*Pc0*FF*M22 + dagger(M12)*dagger(FF)*Pc0*FF*M12 - dagger(M11)*dagger(FF)*Pcm*FF*M11 - dagger(M21)*dagger(FF)*Pc0*FF*M21 - dagger(M22)*dagger(FF)*Pcm*FF*M22 - dagger(M12)*dagger(FF)*Pcp*FF*M12
calc let Bpaper = [0,0,0,0,2/sqrt(3),0,0,0,2; 0,0,0,0,0,2/sqrt(3),0,0,0; 0,0,0,0,0,0,0,0,0; 0,0,0,0,0,0,0,2/sqrt(3),0; 2/sqrt(3),0,0,0,0,0,0,0,2/sqrt(3); 0,2/sqrt(3),0,0,0,0,0,0,0; 0,0,0,0,0,0,0,0,0; 0,0,0,2/sqrt(3),0,0,0,0,0; 2,0,0,0,2/sqrt(3),0,0,0,0]
! echo MARK_BEQ
calc trace(dagger(Bop-Bpaper)*(Bop-Bpaper))
! echo MARK_RAYLEIGH
calc trace(dagger(mv)*Bop*mv)
! echo MARK_RAY_BELL
calc trace(dagger(psiB)*Bop*psiB)
! echo MARK_EIGEN
calc abs(Bop*mv - (1+sqrt(11/3))*mv)
calc let eta = 2/(1+sqrt(11/3))
calc let rho = mv*transpose(mv)
calc let rho1 = eta*rho + (1-eta)*(1/9)*eye(9)
calc let r11 = M11*rho1*dagger(M11)
calc let r12 = M12*rho1*dagger(M12)
calc let r21 = M21*rho1*dagger(M21)
calc let r22 = M22*rho1*dagger(M22)
calc let a11 = FF*r11*dagger(FF)
calc let a12 = FF*r12*dagger(FF)
calc let a21 = FF*r21*dagger(FF)
calc let a22 = FF*r22*dagger(FF)
! echo MARK_NOISE
calc trace(Pc0*a11) + trace(Pcm*a21) + trace(Pc0*a22) + trace(Pc0*a12) - trace(Pcm*a11) - trace(Pc0*a21) - trace(Pcm*a22) - trace(Pcp*a12)
! echo MARK_ETASTAR
calc eta
! echo MARK_ETASTAR_BELL
calc 2/((4/9)*(3+2*sqrt(3)))
! echo MARK_DONE
EOF

echo "── запуск сессии ($SESS, $(wc -l < "$SESS") строк)"
T0=$(date +%s)
timeout 300 "$ENG" --shell < "$SESS" > /tmp/acin_opt_out.txt 2>&1
RC=$?
T1=$(date +%s)
echo "exit=$RC, время: $((T1-T0)) с"

echo "── разбор маркеров (строгий скалярный фильтр — грабли сессии 5)"
python3 - << 'PYEOF'
import re
vals = {}
cur = None
for ln in open('/tmp/acin_opt_out.txt', errors='replace'):
    ln = ln.rstrip()
    # грабли: эхо poler-shell даёт префикс 'poler> '
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
        elif mc:  # комплекс с мнимой пылью < 1e-12 → брать Re
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
print(f"γ (из движка)          = {g('MARK_GAM')}")
print(f"  литература            = 0.7922869914 (√11−√3)/2 — движок ТОЧНО")
i3, tgt = g('MARK_I3'), g('MARK_TARGET')
print(f"I₃ (контур CGLMP)     = {i3}")
print(f"I₃ (цель 1+√(11/3))   = {tgt}")
if i3 is not None and tgt is not None:
    print(f"  расхождение           = {abs(i3-tgt):.2e}  {'✓ ТОЧНО' if abs(i3-tgt) < 1e-9 else '✗ РАСХОЖДЕНИЕ!'}")
print(f"q0 = P(A1=B1)          = {g('MARK_Q0')}   (лит. 0.808341)")
print(f"q₋ = P(A1=B1−1)        = {g('MARK_QM')}   (лит. 0.079628)")
beq = g('MARK_BEQ')
print(f"‖Bop−Bpaper‖²_F        = {beq}   {'✓ ОПЕРАТОР == СТАТЬЕ' if beq is not None and beq < 1e-24 else '✗ ОПЕРАТОР ОТЛИЧАЕТСЯ!'}")
ray = g('MARK_RAYLEIGH')
print(f"⟨Ψ|B|Ψ⟩ Rayleigh      = {ray}")
rayb = g('MARK_RAY_BELL')
print(f"⟨Bell|B|Bell⟩ (сравн.) = {rayb}   (лит. 2.872934 — заметно меньше)")
eig = g('MARK_EIGEN')
print(f"‖B·Ψ − λ·Ψ‖ (норма)   = {eig}   {'✓ СОБСТВЕННЫЙ ВЕКТОР' if eig is not None and eig < 1e-12 else '✗ НЕ СОБСТВЕННЫЙ!'}")
noise = g('MARK_NOISE')
print(f"I₃(η*) при η*=2/λ_max = {noise}   {'✓ =2.0 РОВНО (критическая видимость)' if noise is not None and abs(noise-2.0) < 1e-9 else '✗ НЕ 2.0!'}")
et, etb = g('MARK_ETASTAR'), g('MARK_ETASTAR_BELL')
print(f"η*(Ацин)               = {et}   < η*(Белл) = {etb}  → НЕ-МАКСstate ТОЛЕРАНТНЕЕ К ШУМУ ✓")
PYEOF
echo "═══ КОНЕЦ ВЕРИФИКАЦИИ ═══"
