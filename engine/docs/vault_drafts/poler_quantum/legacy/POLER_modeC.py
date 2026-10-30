from qiskit import QuantumCircuit, transpile
from qiskit_aer import Aer
import numpy as np

def POLER_modeC_step(qc, n=10, kappa=0.9, gamma=0.7):
    gamma_adapt = gamma * np.exp(-abs(np.sin(kappa * np.pi)))  # адаптивная стабилизация
    qc.ry(np.arccos(kappa), 0)
    qc.rx(gamma_adapt, 1)
    qc.cx(0, 1)
    qc.measure_all()
    return qc

qc = QuantumCircuit(2)
qc = POLER_modeC_step(qc, n=10, kappa=0.8, gamma=0.6)

print(qc)

sim = Aer.get_backend('aer_simulator')
compiled = transpile(qc, sim)
result = sim.run(compiled, shots=2048).result()
counts = result.get_counts()

print("Mode C (adaptive) results:", counts)
