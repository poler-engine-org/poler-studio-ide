function updateInterface() {
// Симуляционное время в земных днях
const earthDays = simulationTime;
const earthYears = earthDays / 365.25;

// Период Кроноса вокруг Гелиоса: ~1500 дней
const kronosYears = earthDays / 1517.6;

// Период Кассиопеи вокруг Кроноса: 20 дней
const cassOrbits = earthDays / 20.0;

// Отображение:
document.getElementById('sim-time').textContent =
        earthYears.toFixed(2) + ' земних років';

// Если хотите показать "год Кассиопеи" как 505 дней:
const cassYear = earthDays / 505.0;
document.getElementById('cass-year').textContent = cassYear.toFixed(2);
}
