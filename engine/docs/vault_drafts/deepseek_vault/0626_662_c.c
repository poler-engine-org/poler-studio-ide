// Нормализация весов после обновления
float norm = sqrt(sum of weights^2);
if (norm > MAX_NORM) weights *= MAX_NORM / norm;
