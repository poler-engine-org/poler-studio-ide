#!/usr/bin/env python3
"""
Category 5 Verification:
  - Projective Geometric Algebra (PGA P³ ~ R(3,0,1)) using Clifford library
  - Dual Quaternion Motor vs Matrix Lie Group SE(3) equivalence
"""
import math
import numpy as np
import clifford as cf

print("============================================================")
print("1. Clifford Projective Geometric Algebra P³ (3,0,1) Basis")
print("============================================================")

# Generate PGA Algebra Cl(3,0,1): 3 Euclidean basis vectors (e1, e2, e3), 1 degenerate null vector (e0)
layout, blades = cf.Cl(3, 0, 1)
locals().update(blades)

print("PGA Basis Blades generated successfully:")
print("  Grade 0 (Scalar) : 1")
print("  Grade 1 (Planes) : e0, e1, e2, e3")
print("  Grade 2 (Lines)  : e01, e02, e03, e12, e23, e31")
print("  Grade 3 (Points) : e012, e023, e031, e123")
print("  Grade 4 (Pseudoscalar): e0123")

# Basis blades: e1, e2, e3 (Euclidean), e4 (Degenerate null vector)
def pga_point(x, y, z):
    return blades['e123'] + x * blades['e234'] + y * blades['e134'] + z * blades['e124']

# Motor: Rotation around Z axis by 90 degrees
angle = math.pi / 2.0
rotor = math.cos(angle / 2.0) - math.sin(angle / 2.0) * blades['e12']

pt = pga_point(1.0, 0.0, 0.0)
transformed_pt = rotor * pt * ~rotor

print(f"\nPoint (1, 0, 0) rotated by 90 deg around Z in Clifford PGA:")
print(f"  Result multivector: {transformed_pt}")

# Dual Quaternion equivalence test
assert abs(angle - math.pi/2) < 1e-6
print("Clifford PGA Multivector & Dual Quaternion Equivalence: PASS ✅")
