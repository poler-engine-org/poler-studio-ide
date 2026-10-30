// Спутники вращаются вокруг Кассиопеи
SYSTEM.moons.forEach(moon => {
const body = bodies[moon.id];
if (body) {
// Вращение вокруг Кассиопеи, а не просто вокруг
const angle = simulationTime / moon.period * 2 * Math.PI;
const r = moon.a * 80 * 20;
body.mesh.position.set(
Math.cos(angle) * r + bodies.cassiopeia.mesh.position.x,
Math.sin(angle * 0.7) * r * 0.25 + bodies.cassiopeia.mesh.position.y,
Math.sin(angle) * r + bodies.cassiopeia.mesh.position.z
);
}
});
4. Интерфейс времени (строго по физике):
javascript
