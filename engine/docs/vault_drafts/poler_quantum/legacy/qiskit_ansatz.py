# ==============================================================================
# POLER-QUANTUM: Qiskit Statevector & ERI Molecular Integral Interface
# ==============================================================================

import numpy as np
from qiskit import QuantumCircuit
from qiskit.quantum_info import Statevector

def build_poler_ansatz(num_qubits=4, theta_angles=None):
    if theta_angles is None:
        theta_angles = [np.pi / 4.0] * num_qubits
    
    qc = QuantumCircuit(num_qubits)
    for i in range(num_qubits):
        qc.ry(theta_angles[i], i)
    for i in range(num_qubits - 1):
        qc.cz(i, i + 1)
    return qc

def simulate_h_psi_ground_state():
    qc = build_poler_ansatz(4)
    state = Statevector.from_instruction(qc)
    print("Statevector norm:", np.linalg.norm(state.data))
    return state

if __name__ == "__main__":
    print("Simulating POLER[Psi] Quantum Ansatz...")
    simulate_h_psi_ground_state()
