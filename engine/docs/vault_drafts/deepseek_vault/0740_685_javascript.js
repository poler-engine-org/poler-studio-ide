// Функция расчета периода для любой планеты
function calculateOrbitalPeriod(semiMajorAxisAU, starMass = 0.2) {
// T² ∝ a³/M
// Для Земли: a=1 а.е., M=1 M☉, T=365.25 дней
const T_earth = 365.25;
const T = T_earth * Math.sqrt(
Math.pow(semiMajorAxisAU, 3) / starMass
);
return T;
}

// Примеры:
// Кронос (1.5 а.е., M=0.2): T ≈ 1500 дней
// Эфир (10 а.е., M=0.2): T ≈ 35496 дней (~97 лет)
3. Планетарная система (9 планет):
javascript
