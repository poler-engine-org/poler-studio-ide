// M = Jc · Jc^T  (symmetric positive semi‑definite)
var M = Matrix(N).matmul(Jc, JcT);

// Tikhonov regularization: M_reg = M + δ·I
const delta: f64 = 1e-8;
for (0..N) |i| {
M.data[i][i] = M.data[i][i] + delta;
}
