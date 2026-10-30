#!/usr/bin/env python3
"""poler_v6.py — Complete reference implementation of POLER v6 cycle."""
import numpy as np

class PolerV6Engine:
    def __init__(self, dim=128, eta=0.05, gamma=0.5, rho=0.9):
        self.dim = dim
        self.eta = eta
        self.gamma = gamma
        self.rho = rho
        self.memory = []

    def evolve(self, p, observation, forbidden=False):
        if forbidden:
            return p
        obs = np.tanh(observation)
        grad_f = 2.0 * (p - obs)
        grad_eps = np.zeros_like(p)
        for k, past in enumerate(reversed(self.memory[-8:])):
            grad_eps += (self.rho ** (k + 1)) * (p - past)
        
        dp = -grad_f + self.gamma * grad_eps
        p_next = np.clip(p + self.eta * dp, -1.0, 1.0)
        self.memory.append(obs)
        return p_next
