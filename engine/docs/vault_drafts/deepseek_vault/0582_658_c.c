R = V
for i in range(max_coeff):
best_idx = argmax dot(A[i], R)
if dot < threshold: break
coeff = dot
R -= coeff * A[best_idx]
