// Пояс астероїдів (між планетою 6 та Кроносом)
const asteroidBelt = new THREE.Group();
scene.add(asteroidBelt);

const innerRadius = SYSTEM.asteroidBelt.innerRadius * 80;
const outerRadius = SYSTEM.asteroidBelt.outerRadius * 80;

for (let i = 0; i < SYSTEM.asteroidBelt.count; i++) {
const r = innerRadius + Math.random() * (outerRadius - innerRadius);
const theta = Math.random() * Math.PI * 2;
const y = (Math.random() - 0.5) * 8;

const asteroidGeo = new THREE.SphereGeometry(0.15 + Math.random() * 0.25, 12, 12);
const asteroidMat = new THREE.MeshStandardMaterial({
color: SYSTEM.asteroidBelt.color,
roughness: 0.9,
metalness: 0.1
});
const asteroid = new THREE.Mesh(asteroidGeo, asteroidMat);

asteroid.position.set(
Math.cos(theta) * r,
y,
Math.sin(theta) * r
);

asteroid.rotation.set(
Math.random() * Math.PI,
Math.random() * Math.PI,
Math.random() * Math.PI
);

asteroid.userData = {
orbitSpeed: 0.002 + Math.random() * 0.003,
baseAngle: theta
};

asteroidBelt.add(asteroid);
}

Оновимо резонансні лінії, додавши їх для всіх резонансів:

javascript
