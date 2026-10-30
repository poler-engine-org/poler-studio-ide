'use client';

import { useRef, useMemo } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import { OrbitControls, Sphere } from '@react-three/drei';
import * as THREE from 'three';

function SceneContent() {
const meshRef = useRef<THREE.Mesh>(null);

// Мемоизируем геометрию и материал, чтобы не создавать заново
const geometry = useMemo(() => new THREE.SphereGeometry(1, 64, 64), []);
const material = useMemo(() => new THREE.MeshStandardMaterial({ color: 'orange' }), []);

useFrame((state, delta) => {
if (meshRef.current) {
meshRef.current.rotation.y += delta * 0.5;
}
});

return (
<mesh ref={meshRef} geometry={geometry} material={material} />
);
}

export default function FrederiteScene() {
return (
<Canvas camera={{ position: [0, 2, 5] }}>
<ambientLight intensity={0.5} />
<pointLight position={[10, 10, 10]} />
<SceneContent />
<OrbitControls enableZoom={true} enableRotate={true} />
</Canvas>
);
}
