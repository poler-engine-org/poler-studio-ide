from qiskit import QuantumCircuit, transpile
from qiskit_aer import Aer
from qiskit.visualization import plot_histogram
import numpy as np
import matplotlib.pyplot as plt

def POLER_quantum_step(qc, n=10, kappa=1.0, gamma=0.5):
    # Гейт вращения (ε)
    qc.ry(np.arccos(kappa), 0)
    # Резонансная связь (R[n])
    qc.cx(0, 1)
    # Измерение
    qc.measure_all()
    return qc

# Создаём квантовую схему
qc = QuantumCircuit(2)
qc = POLER_quantum_step(qc, n=10, kappa=0.8, gamma=0.5)

# Печатаем схему
print(qc)

# Запуск симуляции через AerSimulator
sim = Aer.get_backend('aer_simulator')
compiled = transpile(qc, sim)
result = sim.run(compiled, shots=1024).result()
counts = result.get_counts()

# Выводим результаты
print(counts)

# Строим гистограмму
plot_histogram(counts)
plt.show()
