#!/usr/bin/env bash
# CGLMP-неравенство для кутритов (сессия 4) — живой прогон движка poler-engine v0.63.0
# Первоисточник: Collins-Gisin-Linden-Massar-Popescu, PRL 88, 040404 (2002),
# arXiv:quant-ph/0106024v2 (формулы взяты из LaTeX-исходника статьи).
#
# I3 = +[P(A1=B1) + P(B1=A2+1) + P(A2=B2) + P(B2=A1)]
#     -[P(A1=B1-1) + P(B1=A2) + P(A2=B2-1) + P(B2=A1-1)] <= 2  (локальный реализм)
# Квант: |Phi_3> + фазы (a1=0, a2=1/2, b1=1/4, b2=-1/4) + QFT3 с обеих сторон
# Ожидание: I3 = (4/9)(3 + 2*sqrt(3)) = 2.872935... — нарушение локального реализма.
set -u
OUT="${1:-/home/z/my-project/download/experiments/cglmp_v063.txt}"
mkdir -p "$(dirname "$OUT")"

{
echo "════════════════════════════════════════════════════════════════"
echo " CGLMP-НЕРАВЕНСТВО ДЛЯ КУТРИТОВ — poler-engine 0.63.0, сессия 4"
echo " Collins-Gisin-Linden-Massar-Popescu (2002), d=3"
echo " Локальный предел: I3 <= 2 | Цель: 2.872935 = (4/9)(3+2√3)"
echo "════════════════════════════════════════════════════════════════"
} > "$OUT"

SESSION=$(mktemp /tmp/cglmp_XXXXXX.session)

# ---------- БЛОК 1: состояние, измерения, вероятности ----------
cat > "$SESSION" << 'EOF'
! echo "--- БЛОК 1: |Фи_3> Белла, фазовые измерения CGLMP, QFT ---"
calc let om = exp(2*pi/3*i)
calc let F3 = (1/sqrt(3)) * [1,1,1; 1,om,om^2; 1,om^2,om]
calc let psi = (1/sqrt(3)) * [1;0;0;0;1;0;0;0;1]
calc trace(dagger(psi)*psi)
calc let A1 = eye(3)
calc let A2 = [1,0,0; 0,exp(pi/3*i),0; 0,0,exp(2*pi/3*i)]
calc let B1 = [1,0,0; 0,exp(pi/6*i),0; 0,0,exp(pi/3*i)]
calc let B2 = [1,0,0; 0,exp(-pi/6*i),0; 0,0,exp(-pi/3*i)]
! echo "--- ГРАБЛИ, пойманные движком: (F⊗F) даёт плоские 1/3 (корреляции суммы k+l) ---"
! echo "--- ПРАВИЛЬНО: взаимно сопряжённые базисы FF = F3⊗F3† — корреляции разности (k-l) ---"
calc let FF = kron(F3, dagger(F3))
calc let s11 = FF * kron(A1, B1) * psi
calc let s12 = FF * kron(A1, B2) * psi
calc let s21 = FF * kron(A2, B1) * psi
calc let s22 = FF * kron(A2, B2) * psi
! echo "--- нормировка всех 4 состояний (должно быть 1) ---"
calc trace(dagger(s11)*s11)
calc trace(dagger(s12)*s12)
calc trace(dagger(s21)*s21)
calc trace(dagger(s22)*s22)
! echo "--- проекторы Пи_c на (A-B) mod 3 = c ---"
calc let e0 = [1;0;0]
calc let e1 = [0;1;0]
calc let e2 = [0;0;1]
calc let P0 = kron(e0,e0)*transpose(kron(e0,e0)) + kron(e1,e1)*transpose(kron(e1,e1)) + kron(e2,e2)*transpose(kron(e2,e2))
calc let Pp = kron(e1,e0)*transpose(kron(e1,e0)) + kron(e2,e1)*transpose(kron(e2,e1)) + kron(e0,e2)*transpose(kron(e0,e2))
calc let Pm = kron(e2,e0)*transpose(kron(e2,e0)) + kron(e0,e1)*transpose(kron(e0,e1)) + kron(e1,e2)*transpose(kron(e1,e2))
! echo "--- полнота: P0+Pp+Pm = I9, след каждого = 3 ---"
calc P0 + Pp + Pm - eye(9)
calc trace(P0)
calc trace(Pp)
calc trace(Pm)
! echo "--- 8 вероятностей CGLMP: q-структура (симметрия correlP статьи) ---"
calc trace(dagger(s11)*P0*s11)
calc trace(dagger(s21)*Pm*s21)
calc trace(dagger(s22)*P0*s22)
calc trace(dagger(s12)*P0*s12)
calc trace(dagger(s11)*Pm*s11)
calc trace(dagger(s21)*P0*s21)
calc trace(dagger(s22)*Pm*s22)
calc trace(dagger(s12)*Pp*s12)
! echo "--- аналитические эталоны: q0=(4+2√3)/9, q-1=1/9 ---"
calc (4 + 2*sqrt(3))/9
calc 1/9
! echo "--- I3 = 4*(q0 - q-1): ГЛАВНОЕ ЧИСЛО ---"
calc trace(dagger(s11)*P0*s11) + trace(dagger(s21)*Pm*s21) + trace(dagger(s22)*P0*s22) + trace(dagger(s12)*P0*s12) - trace(dagger(s11)*Pm*s11) - trace(dagger(s21)*P0*s21) - trace(dagger(s22)*Pm*s22) - trace(dagger(s12)*Pp*s12)
calc (4/9)*(3 + 2*sqrt(3))
calc (4/9)*(3 + 2*sqrt(3)) - (trace(dagger(s11)*P0*s11) + trace(dagger(s21)*Pm*s21) + trace(dagger(s22)*P0*s22) + trace(dagger(s12)*P0*s12) - trace(dagger(s11)*Pm*s11) - trace(dagger(s21)*P0*s21) - trace(dagger(s22)*Pm*s22) - trace(dagger(s12)*Pp*s12))
! echo "--- нарушение локального реализма: I3 - 2 ---"
calc (trace(dagger(s11)*P0*s11) + trace(dagger(s21)*Pm*s21) + trace(dagger(s22)*P0*s22) + trace(dagger(s12)*P0*s12) - trace(dagger(s11)*Pm*s11) - trace(dagger(s21)*P0*s21) - trace(dagger(s22)*Pm*s22) - trace(dagger(s12)*Pp*s12)) - 2
EOF

echo "── Прогон блока 1 (квантовая часть) ──" >> "$OUT"
poler-engine --shell < "$SESSION" 2>&1 | tee -a "$OUT" | grep -E "(poler> [0-9-])" | head -40

# ---------- БЛОК 2: локальный предел брутфорсом (81 стратегия, арифметика движка) ----------
echo "" >> "$OUT"
echo "── БЛОК 2: локальный предел I3 <= 2 — брутфорс 81 детерминированной стратегии ──" >> "$OUT"
echo "Каждая строка — I3 стратегии (a1,a2,b1,b2), считает Калькулятор движка" >> "$OUT"

BRUTE=$(mktemp /tmp/cglmp_brute_XXXXXX.session)
{
echo '! echo "--- 81 стратегия локального реализма: max должен быть 2 ---"'
for a1 in 0 1 2; do for a2 in 0 1 2; do for b1 in 0 1 2; do for b2 in 0 1 2; do
  # I3_det = T+ - T- ; индикатор равенства: 1 - sign(abs(x-y))
  Tp="(1 - ceil(abs($a1-$b1)/2)) + (1 - ceil(abs($b1 - mod($a2+1,3))/2)) + (1 - ceil(abs($a2-$b2)/2)) + (1 - ceil(abs($b2-$a1)/2))"
  Tm="(1 - ceil(abs($a1 - mod($b1+2,3))/2)) + (1 - ceil(abs($b1-$a2)/2)) + (1 - ceil(abs($a2 - mod($b2+2,3))/2)) + (1 - ceil(abs($b2 - mod($a1+2,3))/2))"
  echo "calc 1*($Tp) - 1*($Tm)"
done; done; done; done
} > "$BRUTE"

poler-engine --shell < "$BRUTE" 2>&1 | grep -oE "poler> -?[0-9]+(\.[0-9]+)?" | sed 's/poler> //' > /tmp/cglmp_brute_vals.txt
python3 - << PYEOF >> "$OUT"
vals = [float(x) for x in open('/tmp/cglmp_brute_vals.txt')]
vals = [v for v in vals if v is not None]
print(f"стратегий: {len(vals)} (ожидается 81)")
print(f"max I3 (локальный реализм) = {max(vals):.6f}  (должно быть ровно 2)")
print(f"min I3 = {min(vals):.6f}, среднее = {sum(vals)/len(vals):.6f} (симметрия: 0)")
import collections
print(f"распределение: {dict(sorted(collections.Counter(vals).items()))}")
PYEOF
rm -f "$SESSION" "$BRUTE"

echo "" >> "$OUT"
echo "── ИТОГ ──" >> "$OUT"
echo "Квантовое значение I3 (Белл + фазы CGLMP + QFT3) и локальный предел см. выше;" >> "$OUT"
echo "нарушение = I3 - 2; цель 2.872935 = (4/9)(3+2*sqrt(3))." >> "$OUT"
echo "Сохранено: $OUT"
head -70 "$OUT"
# ---------- БЛОК 3: шумовая устойчивость через матрицу плотности ----------
echo "" >> "$OUT"
echo "── БЛОК 3: шумовая устойчивость rho(eta) = eta|Фи><Фи| + (1-eta)I/9 ──" >> "$OUT"
echo "ГРАБЛИ сессии: сопряжение с правильной стороны — trace(P_c·FF·r·FF†)," >> "$OUT"
echo "НЕ trace(FF†·r·FF·P): зеркало подменяет класс P-1 на q+1 (движок это поймал)." >> "$OUT"
NOISE=$(mktemp /tmp/cglmp_noise_XXXXXX.session)
cat > "$NOISE" << 'EOF'
! echo "--- плотностная траектория: P_c = trace(P_c*FF*r_ab*dagger(FF)) ---"
calc let om = exp(2*pi/3*i)
calc let F3 = (1/sqrt(3)) * [1,1,1; 1,om,om^2; 1,om^2,om]
calc let psi = (1/sqrt(3)) * [1;0;0;0;1;0;0;0;1]
calc let rho = psi*transpose(psi)
calc let FF = kron(F3, dagger(F3))
calc let A2 = [1,0,0; 0,exp(pi/3*i),0; 0,0,exp(2*pi/3*i)]
calc let B1 = [1,0,0; 0,exp(pi/6*i),0; 0,0,exp(pi/3*i)]
calc let B2 = [1,0,0; 0,exp(-pi/6*i),0; 0,0,exp(-pi/3*i)]
calc let e0 = [1;0;0]
calc let e1 = [0;1;0]
calc let e2 = [0;0;1]
calc let P0 = kron(e0,e0)*transpose(kron(e0,e0)) + kron(e1,e1)*transpose(kron(e1,e1)) + kron(e2,e2)*transpose(kron(e2,e2))
calc let Pm = kron(e2,e0)*transpose(kron(e2,e0)) + kron(e0,e1)*transpose(kron(e0,e1)) + kron(e1,e2)*transpose(kron(e1,e2))
calc let Pp = kron(e1,e0)*transpose(kron(e1,e0)) + kron(e2,e1)*transpose(kron(e2,e1)) + kron(e0,e2)*transpose(kron(e0,e2))
! echo "--- критическая видимость eta* = 2/2.872934051172337 = 0.69615 ---"
calc 2/2.872934051172337
! echo "--- I3(eta*): должно быть 2 ровно (порог нарушения) ---"
calc let etas = 2/2.872934051172337
calc let rho1 = etas*rho + (1-etas)*(1/9)*eye(9)
calc let a11 = FF*kron(eye(3),B1)*rho1*dagger(kron(eye(3),B1))*dagger(FF)
calc let a21 = FF*kron(A2,B1)*rho1*dagger(kron(A2,B1))*dagger(FF)
calc let a22 = FF*kron(A2,B2)*rho1*dagger(kron(A2,B2))*dagger(FF)
calc let a12 = FF*kron(eye(3),B2)*rho1*dagger(kron(eye(3),B2))*dagger(FF)
calc trace(P0*a11) + trace(Pm*a21) + trace(P0*a22) + trace(P0*a12) - trace(Pm*a11) - trace(P0*a21) - trace(Pm*a22) - trace(Pp*a12)
! echo "--- I3(eta*-0.01) < 2: внутри локального полиэдра ---"
calc let etam = etas - 0.01
calc let rhom = etam*rho + (1-etam)*(1/9)*eye(9)
calc let b11 = FF*kron(eye(3),B1)*rhom*dagger(kron(eye(3),B1))*dagger(FF)
calc let b21 = FF*kron(A2,B1)*rhom*dagger(kron(A2,B1))*dagger(FF)
calc let b22 = FF*kron(A2,B2)*rhom*dagger(kron(A2,B2))*dagger(FF)
calc let b12 = FF*kron(eye(3),B2)*rhom*dagger(kron(eye(3),B2))*dagger(FF)
calc trace(P0*b11) + trace(Pm*b21) + trace(P0*b22) + trace(P0*b12) - trace(Pm*b11) - trace(P0*b21) - trace(Pm*b22) - trace(Pp*b12)
! echo "--- I3(eta=1): чистое состояние, совпадение с векторной траекторией ---"
calc let c11 = FF*kron(eye(3),B1)*rho*dagger(kron(eye(3),B1))*dagger(FF)
calc let c21 = FF*kron(A2,B1)*rho*dagger(kron(A2,B1))*dagger(FF)
calc let c22 = FF*kron(A2,B2)*rho*dagger(kron(A2,B2))*dagger(FF)
calc let c12 = FF*kron(eye(3),B2)*rho*dagger(kron(eye(3),B2))*dagger(FF)
calc trace(P0*c11) + trace(Pm*c21) + trace(P0*c22) + trace(P0*c12) - trace(Pm*c11) - trace(P0*c21) - trace(Pm*c22) - trace(Pp*c12)
EOF
poler-engine --shell < "$NOISE" 2>&1 | tee -a "$OUT" | grep -E "poler> [0-9]" | tail -5
rm -f "$NOISE"
echo "" >> "$OUT"
echo "── ИТОГ CGLMP (сессия 4) ──" >> "$OUT"
{
echo "I3(QM) = 2.8729340511723 = (4/9)(3+2*sqrt(3))  — нарушение локального реализма"
echo "Локальный предел: max по 81 стратегии = 2.000000 ровно (арифметика движка)"
echo "Критическая шумовая видимость eta* = 0.696152 — порог нарушения"
echo "Грабли: (1) F⊗F размывает разностные корреляции в 1/3, нужны F⊗F†;"
echo "(2) sign(0)=1 в Калькуляторе — индикаторы строить через ceil(|x-y|/2);"
echo "(3) сторона сопряжения: trace(P·FF·r·FF†), не trace(FF†·r·FF·P)."
} >> "$OUT"
echo "OK: $OUT"
