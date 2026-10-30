"""poler_toolkit/core.py — Deep text analysis core on POLER[Psi]"""
import numpy as np

class PolerAnalyzer:
    def __init__(self, epsilon_threshold=0.85):
        self.epsilon_threshold = epsilon_threshold

    def compute_shannon_entropy(self, tokens):
        from collections import Counter
        import math
        counts = Counter(tokens)
        total = len(tokens)
        return -sum((c / total) * math.log2(c / total) for c in counts.values()) if total > 0 else 0.0

    def calculate_epsilon_window(self, window_tokens, global_freqs):
        # Epsilon density calculation
        score = sum(math.log(100000.0 / max(global_freqs.get(t, 1), 1)) ** 2 for t in window_tokens)
        return score
