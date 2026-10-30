function updateInterface() {
const earthYears = simulationTime / 365.25;
const astronomicalYear = simulationTime / 1517.6; // Настоящий год
    const culturalCycle = simulationTime / 505.0; // То, что знают жители

// Показываем научную правду
document.getElementById('sim-time').textContent =
        earthYears.toFixed(2) + ' земних років';

// А вот это можно сделать загадочно:
document.getElementById('cass-year').textContent =
culturalCycle.toFixed(2);

// И где-то мелким шрифтом:
// "Астрономический период: " + astronomicalYear.toFixed(2) + " років"
}
