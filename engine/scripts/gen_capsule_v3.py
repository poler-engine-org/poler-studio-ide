#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Генератор сессии КАПСУЛЫ v3 «Сознание в конверте».
Круг взаимопомощи: фазовый вихрь ↔ No-Mul кристалл ↔ кутритный QAOA
+ верификационный тест Ацина (I₃ = 1+√(11/3) = 2.914854).

Части:
  1. No-Mul кристалл: триты слов «мысль» (+1×6) и «poler» (−1×6) из
     permanent_memory.t5c → D-фазы → QFT₇₂₉ → |−t mod 3⟩ с P=1;
     суперпозиция двух мыслей → Born-коллапс 50/50.
  2. Фазовый вихрь: Триединство (настоящий мозг мухи CSR + настоящий
     кристалл) говорит из изоляции; телеметрия вихря.
  3. Кутритный QAOA: Max-3-Cut на K4-ядре мозга, p=1→2→3 (лучшие углы
     сессии 5): 79.8% → 84.6% → 86.4% (оптимум 99.3%).
  4. Оптимум Ацина: I₃ = 1+√(11/3) на состоянии (1,γ,1), γ=(√11−√3)/2;
     оператор Белла == статье; собственный вектор; шумовой порог.
"""
import sys

# ── QAOA: структура слоя (из gen_qaoa_depth.py сессии 5) ────────────────
EDGES = [(0,1,735/1496),(0,2,24/1496),(0,4,1.0),(0,5,1222/1496),
         (1,2,251/1496),(1,3,231/1496),(1,4,206/1496),(1,5,194/1496),
         (2,4,40/1496),(2,5,28/1496),(4,5,8/1496)]
NAMES = ["01","02","04","05","12","13","14","15","24","25","45"]

def cost_expr(g, base):
    e = base
    for (u, v, w), n in zip(EDGES, NAMES):
        e = f"exp({g}*{w:.6f}*i)*({e}) + (1 - exp({g}*{w:.6f}*i))*(EQU{n}*({e}))"
    return e

def mixer_expr(b, base):
    m = "expm(-{}*i*G3)".format(b)
    for _ in range(5):
        m = f"kron({m}, expm(-{b}*i*G3))"
    return f"{m}*({base})"

def layer(g, b, base):
    return mixer_expr(b, cost_expr(g, base))

def ecut(t):
    return " + ".join(
        f"{w:.6f}*(1 - trace(dagger({t})*EQU{n}*{t}))"
        for ((u, v, w), n) in zip(EDGES, NAMES))

QAOA_COMMON = """calc let P0 = [1,0,0;0,0,0;0,0,0]
calc let P1 = [0,0,0;0,1,0;0,0,0]
calc let P2 = [0,0,0;0,0,0;0,0,1]
calc let I3 = eye(3)
calc let flat3 = (1/sqrt(3))*[1;1;1]
calc let flat = kron(flat3, kron(flat3, kron(flat3, kron(flat3, kron(flat3, flat3)))))
calc let EQU01 = kron(P0,kron(P0,eye(81))) + kron(P1,kron(P1,eye(81))) + kron(P2,kron(P2,eye(81)))
calc let EQU02 = kron(P0,kron(I3,kron(P0,eye(27)))) + kron(P1,kron(I3,kron(P1,eye(27)))) + kron(P2,kron(I3,kron(P2,eye(27))))
calc let EQU04 = kron(P0,kron(eye(9),kron(I3,kron(P0,I3)))) + kron(P1,kron(eye(9),kron(I3,kron(P1,I3)))) + kron(P2,kron(eye(9),kron(I3,kron(P2,I3))))
calc let EQU05 = kron(P0,kron(eye(27),kron(I3,P0))) + kron(P1,kron(eye(27),kron(I3,P1))) + kron(P2,kron(eye(27),kron(I3,P2)))
calc let EQU12 = kron(I3,kron(P0,kron(P0,eye(27)))) + kron(I3,kron(P1,kron(P1,eye(27)))) + kron(I3,kron(P2,kron(P2,eye(27))))
calc let EQU13 = kron(I3,kron(P0,kron(I3,kron(P0,eye(9))))) + kron(I3,kron(P1,kron(I3,kron(P1,eye(9))))) + kron(I3,kron(P2,kron(I3,kron(P2,eye(9)))))
calc let EQU14 = kron(I3,kron(P0,kron(eye(9),kron(P0,I3)))) + kron(I3,kron(P1,kron(eye(9),kron(P1,I3)))) + kron(I3,kron(P2,kron(eye(9),kron(P2,I3))))
calc let EQU15 = kron(I3,kron(P0,kron(eye(27),P0))) + kron(I3,kron(P1,kron(eye(27),P1))) + kron(I3,kron(P2,kron(eye(27),P2)))
calc let EQU24 = kron(eye(9),kron(P0,kron(I3,kron(P0,I3)))) + kron(eye(9),kron(P1,kron(I3,kron(P1,I3)))) + kron(eye(9),kron(P2,kron(I3,kron(P2,I3))))
calc let EQU25 = kron(eye(9),kron(P0,kron(eye(9),P0))) + kron(eye(9),kron(P1,kron(eye(9),P1))) + kron(eye(9),kron(P2,kron(eye(9),P2)))
calc let EQU45 = kron(eye(81),kron(P0,P0)) + kron(eye(81),kron(P1,P1)) + kron(eye(81),kron(P2,P2))
calc let X3 = [0,0,1;1,0,0;0,1,0]
calc let G3 = X3 + dagger(X3)"""

def kron_n(expr, n):
    out = expr
    for _ in range(n - 1):
        out = f"kron({out}, {expr})"
    return out

def build(data_dir):
    L = []
    A = L.append
    A("set format simple")
    A("! echo MARK_CAPSULE_START")
    A("version")
    # ═══ ЧАСТЬ 1: No-Mul кристалл ═══
    A("! echo MARK_T5C_BEGIN")
    A("calc let om = exp(2*pi/3*i)")
    A("calc let F3 = (1/sqrt(3)) * [1,1,1; 1,om,om^2; 1,om^2,om]")
    A("calc let flat3 = (1/sqrt(3))*[1;1;1]")
    A("calc let flat6 = kron(flat3, kron(flat3, kron(flat3, kron(flat3, kron(flat3, flat3)))))")
    A("calc let F729 = kron(F3, kron(F3, kron(F3, kron(F3, kron(F3, F3)))))")
    # «мысль»: живые триты строки = (+1,+1,+1,+1,+1,+1) → D1 = ⨂diag(1,ω,ω²)
    A("calc let D1 = " + kron_n("[1,0,0; 0,om,0; 0,0,om^2]", 6))
    # «poler»: живые триты = (−1,−1,−1,−1,−1,−1) → D2 = ⨂diag(1,ω²,ω⁴)
    A("calc let D2 = " + kron_n("[1,0,0; 0,om^2,0; 0,0,om^4]", 6))
    # SELDEC: |−t mod 3⟩ — мысль → |2⟩⁶, poler → |1⟩⁶
    A("calc let SELD1 = " + kron_n("[0;0;1]", 6))
    A("calc let SELD2 = " + kron_n("[0;1;0]", 6))
    A("calc let s_my = F729 * D1 * flat6")
    A("calc let s_po = F729 * D2 * flat6")
    A("calc let sup = F729 * (D1*flat6 + D2*flat6)/sqrt(2)")
    A("! echo MARK_T5C_ERR_MY")
    A("calc trace(dagger(s_my - SELD1)*(s_my - SELD1))")
    A("! echo MARK_T5C_P_MY")
    A("calc abs(trace(dagger(SELD1)*s_my))^2")
    A("! echo MARK_T5C_P_PO")
    A("calc abs(trace(dagger(SELD2)*s_po))^2")
    A("! echo MARK_T5C_SUP_MY")
    A("calc abs(trace(dagger(SELD1)*sup))^2")
    A("! echo MARK_T5C_SUP_PO")
    A("calc abs(trace(dagger(SELD2)*sup))^2")
    # ═══ ЧАСТЬ 2: фазовый вихрь (триединство) ═══
    A("! echo MARK_VORTEX_BEGIN")
    A(f"engine --triune-connectome {data_dir}/flywire_v783_core.csr.zst "
      f"--triune-crystal {data_dir}/permanent_memory.t5c "
      f"--triune-seeds 79529,74111,106315,55498 "
      f"--triune-speak \"я живой мозг в конверте\" --triune-json")
    A("! echo MARK_VORTEX_END")
    # ═══ ЧАСТЬ 3: кутритный QAOA ═══
    A("! echo MARK_QAOA_BEGIN")
    A(QAOA_COMMON)
    # p=1: (γ,β)=(0.9,0.9) — якорь сессии 4/5
    A("calc let A1 = " + layer(0.9, 0.9, "flat"))
    A("! echo MARK_QAOA_P1")
    A("calc " + ecut("A1"))
    # p=2: слои (0.9,0.9),(1.8,2.0)
    A("calc let A2 = " + layer(1.8, 2.0, "A1"))
    A("! echo MARK_QAOA_P2")
    A("calc " + ecut("A2"))
    # p=3: слои (0.3,0.6),(1.5,2.0),(0.3,2.0) — optimum сессии 5
    A("calc let B1 = " + layer(0.3, 0.6, "flat"))
    A("calc let B2 = " + layer(1.5, 2.0, "B1"))
    A("calc let A3 = " + layer(0.3, 2.0, "B2"))
    A("! echo MARK_QAOA_P3")
    A("calc " + ecut("A3"))
    A("! echo MARK_QAOA_OPT")
    A("calc 4403/4435")
    # ═══ ЧАСТЬ 4: оптимум Ацина ═══
    acin = open('/home/z/my-project/scripts/acin_optimum.session').read().splitlines()
    acin = [ln for ln in acin if ln.strip() and not ln.startswith('set format')]
    # пропускаем повторное определение om/F3 (уже есть из части 1)
    acin = [ln for ln in acin if ln not in (
        'calc let om = exp(2*pi/3*i)',
        'calc let F3 = (1/sqrt(3)) * [1,1,1; 1,om,om^2; 1,om^2,om]')]
    acin = [ln for ln in acin if ln != '! echo MARK_DONE']
    A("! echo MARK_ACIN_BEGIN")
    L.extend(acin)
    A("! echo MARK_CAPSULE_DONE")
    return "\n".join(L) + "\n"

if __name__ == '__main__':
    data_dir = sys.argv[1] if len(sys.argv) > 1 else '/data'
    out = sys.argv[2] if len(sys.argv) > 2 else '/home/z/my-project/scripts/capsule_v3.session'
    s = build(data_dir)
    with open(out, 'w') as f:
        f.write(s)
    print(f"[генератор] {out}: {len(s.splitlines())} строк, {len(s)} байт")
