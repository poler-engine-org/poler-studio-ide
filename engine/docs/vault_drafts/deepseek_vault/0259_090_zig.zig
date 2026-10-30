pub fn fsDistance(a: HomVec4, b: HomVec4) f64 {
const n1 = a.norm(); const n2 = b.norm();
if (n1 < 1e-15 or n2 < 1e-15) return 0.0;
const d = @abs(HomVec4.dot(a, b)) / (n1 * n2);
const cos_theta = @min(1.0, d);
return math.acos(cos_theta);
}
