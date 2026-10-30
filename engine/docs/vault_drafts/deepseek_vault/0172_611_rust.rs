let diff_row = diff.clone().unsqueeze::<2>(); // [1, dim]
let diff_col = diff.clone().unsqueeze::<2>().transpose(); // [dim, 1]
let squared_norm = diff_row.matmul(self.metric.clone()).matmul(diff_col).squeeze(1).squeeze(0);
let length = squared_norm.sqrt().into_scalar();
