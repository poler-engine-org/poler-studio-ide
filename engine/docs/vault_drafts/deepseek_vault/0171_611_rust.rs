let mask = magnitude.greater_elem(threshold).float();
weights.clone() * mask
