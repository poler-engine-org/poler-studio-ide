function calculateTidalHeating(cassiopeia, kronos) {
// Ваши формулы
const G = 6.67430e-11;
const R = cassiopeia.data.radius * 6371000;  // В метрах
const e = cassiopeia.data.eccentricity;
const n = 2 * Math.PI / (cassiopeia.data.period * 86400);  // рад/сек
const M_kronos = kronos.data.mass * 5.9722e24;  // В кг
const a = cassiopeia.data.semiMajorAxis * 149597870.7;  // В метрах
const k2_Q = 0.015;

const powerPerArea = (21/2) * k2_Q * Math.pow(G * M_kronos * R / Math.pow(a, 3), 2) *
(n * Math.pow(R, 3)) * Math.pow(e, 2);

return Math.min(100, powerPerArea * 0.3 * 1e6);  // В процентах
}
