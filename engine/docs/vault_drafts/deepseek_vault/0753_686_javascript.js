// Резонансні лінії
resonanceLines = [];

SYSTEM.planets.forEach(planet => {
if (planet.resonance) {
const targetBody = bodies[planet.resonance.target];
if (targetBody) {
const lineGeometry = new THREE.BufferGeometry().setFromPoints([
new THREE.Vector3(), new THREE.Vector3()
]);
const lineMaterial = new THREE.LineBasicMaterial({
color: planet.resonance.ratio[0] === 2 ? 0xff5555 : 0x55ff55,
transparent: true,
opacity: 0.4
});
const line = new THREE.Line(lineGeometry, lineMaterial);
line.visible = true;
scene.add(line);
resonanceLines.push({
line: line,
source: planet.id,
target: planet.resonance.target
});
}
}
});

Тепер додамо функції для розрахунку припливного нагрівання та радіаційного захисту:

javascript
