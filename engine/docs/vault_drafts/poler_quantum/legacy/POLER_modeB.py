from qiskit import QuantumCircuit, transpile
from qiskit_aer import Aer
import numpy as np

def POLER_modeB_step(qc, n=10, kappa=0.8, gamma=0.5):
    qc.ry(np.arccos(kappa), 0)
    qc.cx(0, 1)
    qc.rz(gamma, 1)
    qc.measure_all()
    return qc

qc = QuantumCircuit(2)
qc = POLER_modeB_step(qc, n=10, kappa=0.8, gamma=0.5)

print(qc)

sim = Aer.get_backend('aer_simulator')
compiled = transpile(qc, sim)
result = sim.run(compiled, shots=1024).result()
counts = result.get_counts()

print("Mode B results:", counts)
