// Створення Ефіра (бурого карлика)
const aetherGeometry = new THREE.SphereGeometry(8, 64, 64); // Менший за Геліос
const aetherMaterial = new THREE.MeshBasicMaterial({
color: 0x8B4513,
transparent: true,
opacity: 0.8
});
const aetherMesh = new THREE.Mesh(aetherGeometry, aetherMaterial);
scene.add(aetherMesh);

bodies.aether = {
mesh: aetherMesh,
data: SYSTEM.aether,
position: new THREE.Vector3(0, 0, 0),
velocity: new THREE.Vector3(0, 0, 0),
angle: Math.random() * Math.PI * 2,
orbitalPeriod: SYSTEM.aether.period
};

// Орбіта Ефіра
const aetherOrbit = createOrbitPath(SYSTEM.aether.semiMajorAxis * 80, SYSTEM.aether.eccentricity, 0x8B4513);
scene.add(aetherOrbit);
aetherOrbit.visible = false;
orbitLines.aether = aetherOrbit;
