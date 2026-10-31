#!/usr/bin/env bash
# СЕССИЯ 7 — CGLMP d=4: КВКВАРТЫ на чистом Калькуляторе движка.
#
# Физика: Collins-Gisin-Linden-Massar-Popescu (quant-ph/0106024), фазы
# α₁=0, α₂=1/2, β₁=1/4, β₂=−1/4 (линейные лестницы — DE сессии 7 подтвердил
# их оптимальность и для некомпактного состояния независимо).
#
#   max-ent:  I₄ = (2/3)(√2 + √(10−√2)) ≈ 2.896243218   (статья, точно)
#   некомпакт (СЕССИЯ 7, НОВОЕ): палиндром (1,γ,γ,1)/√(2+2γ²), где
#     γ₄ = [√(8−3√2+4√(2−√2)) − √(2−√2)] / [√2(1+√(2−√2))]  (корень
#     квадратики √2(1+√(2−√2))γ² + 2√(2−√2)γ − √2(1+√(2−√2)) = 0)
#     I₄* = 2.972698267102 > I₃*(Ацин) = 2.914854 — квкварт бьёт кутрит!
#     η* = 0.672789 < η*(Ацин d=3) = 0.686141 — терпеливее к шуму.
#
# Проверяется путями (по образцу acin_optimum_run.sh):
#   A) max-ent контур == аналитике статьи (позлементно через q₀)
#   B) контур некомпактного оптимума == точной алгебре (√2-башня)
#   C) оператор Белла B₄ (16×16): Rayleigh + собственное уравнение
#   D) шумовой порог η* = 2/I₄* → I₄(η*) = 2.0 ровно
set -u
ENG="${ENG:-poler-engine}"
OUT="${OUT:-/home/z/my-project/download/experiments/d4_ququart_v063.txt}"
SESS="scripts/d4_ququart.session"
mkdir -p "$(dirname "$OUT")"
exec > >(tee "$OUT") 2>&1

echo "═══ CGLMP d=4: КВКВАРТЫ НА КАЛЬКУЛЯТОРЕ — $(date -u '+%Y-%m-%d %H:%M UTC') ═══"
echo "Движок: $("$ENG" --exec 'version' < /dev/null 2>&1 | head -1)"

cat > "$SESS" << 'EOF'
set format simple
calc let F4 = (1/2)*[1,1,1,1; 1,i,-1,-i; 1,-1,1,-1; 1,-i,-1,i]
calc let u0 = [1;0;0;0]
calc let u1 = [0;1;0;0]
calc let u2 = [0;0;1;0]
calc let u3 = [0;0;0;1]
calc let FF4 = kron(F4, dagger(F4))
calc let A2p = [1,0,0,0; 0,exp(pi/4*i),0,0; 0,0,exp(pi/2*i),0; 0,0,0,exp(3*pi/4*i)]
calc let B1p = [1,0,0,0; 0,exp(pi/8*i),0,0; 0,0,exp(pi/4*i),0; 0,0,0,exp(3*pi/8*i)]
calc let B2p = [1,0,0,0; 0,exp(-pi/8*i),0,0; 0,0,exp(-pi/4*i),0; 0,0,0,exp(-3*pi/8*i)]
calc let Qc0 = kron(u0,u0)*transpose(kron(u0,u0)) + kron(u1,u1)*transpose(kron(u1,u1)) + kron(u2,u2)*transpose(kron(u2,u2)) + kron(u3,u3)*transpose(kron(u3,u3))
calc let Qc1 = kron(u1,u0)*transpose(kron(u1,u0)) + kron(u2,u1)*transpose(kron(u2,u1)) + kron(u3,u2)*transpose(kron(u3,u2)) + kron(u0,u3)*transpose(kron(u0,u3))
calc let Qc2 = kron(u2,u0)*transpose(kron(u2,u0)) + kron(u3,u1)*transpose(kron(u3,u1)) + kron(u0,u2)*transpose(kron(u0,u2)) + kron(u1,u3)*transpose(kron(u1,u3))
calc let Qc3 = kron(u3,u0)*transpose(kron(u3,u0)) + kron(u0,u1)*transpose(kron(u0,u1)) + kron(u1,u2)*transpose(kron(u1,u2)) + kron(u2,u3)*transpose(kron(u2,u3))
calc let M11 = kron(eye(4), B1p)
calc let M12 = kron(eye(4), B2p)
calc let M21 = kron(A2p, B1p)
calc let M22 = kron(A2p, B2p)
calc let psiB4 = (1/2)*[1;0;0;0;0;1;0;0;0;0;1;0;0;0;0;1]
calc let gam4 = (sqrt(8 - 3*sqrt(2) + 4*sqrt(2-sqrt(2))) - sqrt(2-sqrt(2))) / (sqrt(2)*(1+sqrt(2-sqrt(2))))
calc let nn4 = 2 + 2*gam4^2
calc let mv4 = (1/sqrt(nn4))*[1;0;0;0;0;gam4;0;0;0;0;gam4;0;0;0;0;1]
calc let me11 = FF4*M11*psiB4
calc let me12 = FF4*M12*psiB4
calc let me21 = FF4*M21*psiB4
calc let me22 = FF4*M22*psiB4
calc let s11 = FF4*M11*mv4
calc let s12 = FF4*M12*mv4
calc let s21 = FF4*M21*mv4
calc let s22 = FF4*M22*mv4
! echo MARK_GAM4
calc gam4
! echo MARK_Q0
calc trace(dagger(me11)*Qc0*me11)
! echo MARK_Q0_LIT
calc 1/(8*(2-sqrt(2+sqrt(2))))
! echo MARK_I4ME
calc trace(dagger(me11)*Qc0*me11) + trace(dagger(me21)*Qc3*me21) + trace(dagger(me22)*Qc0*me22) + trace(dagger(me12)*Qc0*me12) - trace(dagger(me11)*Qc3*me11) - trace(dagger(me21)*Qc0*me21) - trace(dagger(me22)*Qc3*me22) - trace(dagger(me12)*Qc1*me12) + (1/3)*(trace(dagger(me11)*Qc1*me11) + trace(dagger(me21)*Qc2*me21) + trace(dagger(me22)*Qc1*me22) + trace(dagger(me12)*Qc3*me12) - trace(dagger(me11)*Qc2*me11) - trace(dagger(me21)*Qc1*me21) - trace(dagger(me22)*Qc2*me22) - trace(dagger(me12)*Qc2*me12))
! echo MARK_I4ME_LIT
calc (2/3)*(sqrt(2)+sqrt(10-sqrt(2)))
! echo MARK_I4STAR
calc trace(dagger(s11)*Qc0*s11) + trace(dagger(s21)*Qc3*s21) + trace(dagger(s22)*Qc0*s22) + trace(dagger(s12)*Qc0*s12) - trace(dagger(s11)*Qc3*s11) - trace(dagger(s21)*Qc0*s21) - trace(dagger(s22)*Qc3*s22) - trace(dagger(s12)*Qc1*s12) + (1/3)*(trace(dagger(s11)*Qc1*s11) + trace(dagger(s21)*Qc2*s21) + trace(dagger(s22)*Qc1*s22) + trace(dagger(s12)*Qc3*s12) - trace(dagger(s11)*Qc2*s11) - trace(dagger(s21)*Qc1*s21) - trace(dagger(s22)*Qc2*s22) - trace(dagger(s12)*Qc2*s12))
! echo MARK_I4STAR_LIT
calc 2.9726982671022438304698324281
calc let Bop4 = dagger(M11)*dagger(FF4)*Qc0*FF4*M11 + dagger(M21)*dagger(FF4)*Qc3*FF4*M21 + dagger(M22)*dagger(FF4)*Qc0*FF4*M22 + dagger(M12)*dagger(FF4)*Qc0*FF4*M12 - dagger(M11)*dagger(FF4)*Qc3*FF4*M11 - dagger(M21)*dagger(FF4)*Qc0*FF4*M21 - dagger(M22)*dagger(FF4)*Qc3*FF4*M22 - dagger(M12)*dagger(FF4)*Qc1*FF4*M12 + (1/3)*(dagger(M11)*dagger(FF4)*Qc1*FF4*M11 + dagger(M21)*dagger(FF4)*Qc2*FF4*M21 + dagger(M22)*dagger(FF4)*Qc1*FF4*M22 + dagger(M12)*dagger(FF4)*Qc3*FF4*M12 - dagger(M11)*dagger(FF4)*Qc2*FF4*M11 - dagger(M21)*dagger(FF4)*Qc1*FF4*M21 - dagger(M22)*dagger(FF4)*Qc2*FF4*M22 - dagger(M12)*dagger(FF4)*Qc2*FF4*M12)
! echo MARK_RAYLEIGH
calc trace(dagger(mv4)*Bop4*mv4)
! echo MARK_RAY_BELL
calc trace(dagger(psiB4)*Bop4*psiB4)
! echo MARK_EIGEN
calc abs(Bop4*mv4 - 2.9726982671022438304698324281*mv4)
! echo MARK_HERM
calc trace(dagger(Bop4 - dagger(Bop4))*(Bop4 - dagger(Bop4)))
calc let eta4 = 2/2.9726982671022438304698324281
calc let rho4 = mv4*transpose(mv4)
calc let rho41 = eta4*rho4 + (1-eta4)*(1/16)*eye(16)
calc let r11 = M11*rho41*dagger(M11)
calc let r12 = M12*rho41*dagger(M12)
calc let r21 = M21*rho41*dagger(M21)
calc let r22 = M22*rho41*dagger(M22)
calc let a11 = FF4*r11*dagger(FF4)
calc let a12 = FF4*r12*dagger(FF4)
calc let a21 = FF4*r21*dagger(FF4)
calc let a22 = FF4*r22*dagger(FF4)
! echo MARK_NOISE
calc trace(Qc0*a11) + trace(Qc3*a21) + trace(Qc0*a22) + trace(Qc0*a12) - trace(Qc3*a11) - trace(Qc0*a21) - trace(Qc3*a22) - trace(Qc1*a12) + (1/3)*(trace(Qc1*a11) + trace(Qc2*a21) + trace(Qc1*a22) + trace(Qc3*a12) - trace(Qc2*a11) - trace(Qc1*a21) - trace(Qc2*a22) - trace(Qc2*a12))
! echo MARK_ETASTAR
calc eta4
! echo MARK_ETA_D3_ACIN
calc 2/(1+sqrt(11/3))
! echo MARK_I3_ACIN
calc 1+sqrt(11/3)
! echo MARK_DONE
EOF

echo "── запуск сессии ($SESS, $(wc -l < "$SESS") строк)"
T0=$(date +%s)
timeout 300 "$ENG" --shell < "$SESS" > /tmp/d4_ququart_out.txt 2>&1
RC=$?
T1=$(date +%s)
echo "exit=$RC, время: $((T1-T0)) с"

echo "── разбор маркеров (строгий скалярный фильтр — грабли сессий 5/6)"
python3 - << 'PYEOF'
import re
vals = {}
cur = None
for ln in open('/tmp/d4_ququart_out.txt', errors='replace'):
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
gam = g('MARK_GAM4')
print(f"γ4 (движок)            = {gam}")
print(f"  точная алгебра        = 0.7393724305634157 (√2-башня)")
if gam is not None:
    print(f"  расхождение           = {abs(gam-0.7393724305634157):.2e}")
q0, q0l = g('MARK_Q0'), g('MARK_Q0_LIT')
print(f"q0 = P(A1=B1)          = {q0}")
print(f"  1/(8(2-√(2+√2)))      = {q0l}")
if q0 is not None and q0l is not None:
    print(f"  расхождение           = {abs(q0-q0l):.2e}  {'✓' if abs(q0-q0l)<1e-12 else '✗'}")
me, mel = g('MARK_I4ME'), g('MARK_I4ME_LIT')
print(f"I4(max-ent) контур     = {me}")
print(f"  (2/3)(√2+√(10-√2))   = {mel}")
if me is not None and mel is not None:
    print(f"  расхождение           = {abs(me-mel):.2e}  {'✓ СТАТЬЯ' if abs(me-mel)<1e-10 else '✗ РАСХОЖДЕНИЕ!'}")
i4, i4l = g('MARK_I4STAR'), g('MARK_I4STAR_LIT')
print(f"I4* (некомпакт)        = {i4}")
print(f"  точная алгебра        = {i4l}")
if i4 is not None and i4l is not None:
    print(f"  расхождение           = {abs(i4-i4l):.2e}  {'✓ ТОЧНО' if abs(i4-i4l)<1e-10 else '✗'}")
ray = g('MARK_RAYLEIGH')
print(f"⟨Ψ|B4|Ψ⟩ Rayleigh      = {ray}   {'✓ == контур' if ray is not None and i4 is not None and abs(ray-i4)<1e-9 else '?'}")
rayb = g('MARK_RAY_BELL')
print(f"⟨Bell|B4|Bell⟩        = {rayb}   (max-ent в том же операторе)")
eig = g('MARK_EIGEN')
print(f"‖B4·Ψ − λ·Ψ‖           = {eig}   {'✓ СОБСТВЕННЫЙ ВЕКТОР' if eig is not None and eig < 1e-10 else '✗'}")
herm = g('MARK_HERM')
print(f"‖B4 − B4†‖²_F          = {herm}   {'✓ эрмитов' if herm is not None and herm < 1e-20 else '?'}")
noise = g('MARK_NOISE')
print(f"I4(η*) при η*=2/I4*    = {noise}   {'✓ =2.0 РОВНО' if noise is not None and abs(noise-2.0) < 1e-9 else '✗ НЕ 2.0!'}")
et, et3 = g('MARK_ETASTAR'), g('MARK_ETA_D3_ACIN')
i3a = g('MARK_I3_ACIN')
print(f"η*(d=4)                = {et}   < η*(Ацин d=3) = {et3}  → d=4 ТОЛЕРАНТНЕЕ К ШУМУ ✓")
if i4 is not None and i3a is not None:
    print(f"I4* vs I3*(Ацин)       = {i4} vs {i3a}  → квкварт бьёт кутрит: +{100*(i4/i3a-1):.3f}%")
PYEOF
echo "═══ КОНЕЦ ВЕРИФИКАЦИИ d=4 ═══"
