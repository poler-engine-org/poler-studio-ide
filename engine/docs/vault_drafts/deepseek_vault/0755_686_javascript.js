function updateSimulation(delta) {
// ... існуючий код ...

// Оновлення Кассіопеї відносно Кроноса
const kronos = bodies.kronos;
if (kronos) {
cassiopeiaGroup.position.copy(kronos.mesh.position);

const cassPos = calculateOrbitalPosition(bodies.cassiopeia, simulationTime * 5);
cassiopeiaMesh.position.set(cassPos.x * 0.3, cassPos.y * 0.3, cassPos.z * 0.3);

// Розрахунок припливного нагрівання
const tidalHeat = calculateTidalHeating(bodies.cassiopeia, kronos);
bodies.cassiopeia.data.tidal_heating = Math.min(100, tidalHeat * 10);

// Розрахунок радіаційного захисту
const aetherBody = bodies.aether;
if (aetherBody) {
bodies.cassiopeia.data.radiation_protection =
calculateRadiationProtection(bodies.cassiopeia, kronos, aetherBody);
}

// Сезонна температура
const seasonalFactor = Math.sin(simulationTime / 505 * 2 * Math.PI +
SYSTEM.cassiopeia.axial_tilt * Math.PI / 180);
bodies.cassiopeia.data.temperature = 275 + 15 * seasonalFactor +
10 * (bodies.cassiopeia.data.tidal_heating / 100);
}

// ... інші оновлення ...
}
