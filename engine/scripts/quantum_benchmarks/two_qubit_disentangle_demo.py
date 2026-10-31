#!/usr/bin/env python3
"""
POLER Quantum 2-Qubit Entanglement, Bell State & Disentanglement Test
"""
import numpy as np

def main():
    print("=== POLER Quantum 2-Qubit Disentanglement Analysis ===")
    psi0 = np.array([1, 0, 0, 0], dtype=complex)
    H = np.array([[1, 1], [1, -1]]) / np.sqrt(2)
    I = np.eye(2)
    H0 = np.kron(H, I)

    CNOT = np.array([
        [1, 0, 0, 0],
        [0, 1, 0, 0],
        [0, 0, 0, 1],
        [0, 0, 1, 0]
    ])

    # 1. Entanglement
    psi_super = H0 @ psi0
    psi_entangled = CNOT @ psi_super
    rho = np.outer(psi_entangled, np.conj(psi_entangled))
    rho_A = np.trace(rho.reshape(2, 2, 2, 2), axis1=1, axis2=3)
    ev = np.linalg.eigvalsh(rho_A)
    ev = ev[ev > 1e-12]
    s_ent = -np.sum(ev * np.log2(ev))
    print(f"Entangled Bell State |Phi+>: S(A) = {s_ent:.4f} bit")

    # 2. Disentanglement
    psi_dis1 = CNOT @ psi_entangled
    psi_final = H0 @ psi_dis1
    rho_f = np.outer(psi_final, np.conj(psi_final))
    rho_Af = np.trace(rho_f.reshape(2, 2, 2, 2), axis1=1, axis2=3)
    evf = np.linalg.eigvalsh(rho_Af)
    evf = evf[evf > 1e-12]
    s_dis = -np.sum(evf * np.log2(evf)) if len(evf) > 0 else 0.0
    print(f"Disentangled Final State |00>: S(A) = {s_dis:.4f} bit (Clean zero-entropy!)")

if __name__ == '__main__':
    main()
