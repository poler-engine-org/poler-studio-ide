function calculateTidalHeating(cassiopeia, kronos) {
// Формула припливного нагрівання (Peale et al. 1979)
const G = SYSTEM.G;
    const R = cassiopeia.data.radius * 6371000; // радіус у метрах
const e = cassiopeia.data.eccentricity;
    const n = 2 * Math.PI / (cassiopeia.data.period * 86400); // середня рухова частота
const M_kronos = kronos.data.mass * SYSTEM.JUPITER_MASS;
const a = cassiopeia.data.semiMajorAxis * SYSTEM.AU;

// Припливний параметр (для суперземлі)
    const k2_Q = 0.015; // комбінований параметр

// Потужність на одиницю площі
const powerPerArea = (21 / 2) * (k2_Q) * (G * M_kronos * R / (a * a * a))**2 *
(n * R * R * R) * e * e;

    return powerPerArea * 1e-3; // конвертація у прийнятні одиниці
}

function calculateRadiationProtection(cassiopeia, kronos, aether) {
// Модель подвійного магнітного щита
const distToKronos = cassiopeia.data.semiMajorAxis; // а.о.
const distToAether = Math.hypot(
kronos.position.x - aether.position.x,
kronos.position.z - aether.position.z
    ) / 80; // конвертація у а.о.

// Захист від Кроноса (масштабований за відстанню)
const kronosShield = 0.35 * Math.exp(-distToKronos * 50);

// Захист від Ефіра (бурий карлик як магнітний щит)
const aetherShield = 0.6 * Math.min(1.0, 10 / (distToAether * distToAether + 1));

// Загальний захист з урахуванням спалахів Геліоса
const flareFactor = 1.0 + 0.5 * Math.sin(simulationTime / SYSTEM.helios.flare_cycle * 2 * Math.PI);
const reduction = 0.1 * (flareFactor - 1.0);

return Math.max(0.5, Math.min(0.98, kronosShield + aetherShield - reduction)) * 100;
}

Оновимо функцію updateSimulation для розрахунків:

javascript
