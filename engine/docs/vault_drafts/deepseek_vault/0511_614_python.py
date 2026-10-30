def test_conservation():
system = POLERUnifiedSystem(dim=10)
system.gamma0 = 0.0
system.lambda_O = 0.0
p0 = system.p.copy()
for _ in range(100):
system.step()
assert np.allclose(np.linalg.norm(system.p), np.linalg.norm(p0), rtol=1e-3)
