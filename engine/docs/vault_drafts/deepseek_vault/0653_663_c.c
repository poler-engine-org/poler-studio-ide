float* sys_create_diagonal_constraint(int n, float diss_scale) {
float* M = (float*)calloc(n * n, sizeof(float));
for (int i = 0; i < n; i++) M[i * n + i] = -diss_scale;  // -D
return M;
}
