"""pyscf_poler/hf_poler_hijack.py — Жесткий перехват DIIS в PySCF"""
import numpy as np

def poler_diis_replacement(dm_history, fock_history):
    """Temporal Echo R[n] replacement for standard DIIS."""
    if len(dm_history) < 2:
        return fock_history[-1]
    
    # Skew-symmetric resonance J
    delta = dm_history[-1] - dm_history[-2]
    A = dm_history[-2] @ delta.T
    J = A - A.T
    return fock_history[-1] + 0.1 * J
