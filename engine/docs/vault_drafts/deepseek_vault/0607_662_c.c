// Пример: нормализация скрытых->выходных
float sum_sq = 0;
for each link (h->o) sum_sq += w*w;
float norm = sqrtf(sum_sq);
if (norm > 1.0) for each link w /= norm;
