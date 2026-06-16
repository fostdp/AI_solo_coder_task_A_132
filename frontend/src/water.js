import * as THREE from 'three';

export class WaterSurface {
    constructor(radius) {
        this.radius = radius;
        this.time = 0;

        const geometry = new THREE.CircleGeometry(radius, 64);
        const material = new THREE.MeshStandardMaterial({
            color: 0x1e90ff,
            transparent: true,
            opacity: 0.7,
            roughness: 0.1,
            metalness: 0.3,
            side: THREE.DoubleSide,
        });

        this.mesh = new THREE.Mesh(geometry, material);
        this.mesh.rotation.x = -Math.PI / 2;
        this.mesh.receiveShadow = true;

        this.originalPositions = geometry.attributes.position.array.slice();

        const edgeGeometry = new THREE.RingGeometry(radius * 0.95, radius, 64);
        const edgeMaterial = new THREE.MeshBasicMaterial({
            color: 0x00bfff,
            transparent: true,
            opacity: 0.5,
            side: THREE.DoubleSide,
        });
        this.edgeMesh = new THREE.Mesh(edgeGeometry, edgeMaterial);
        this.edgeMesh.rotation.x = -Math.PI / 2;
        this.mesh.add(this.edgeMesh);
    }

    update(delta) {
        this.time += delta;

        const positions = this.mesh.geometry.attributes.position.array;
        const waveHeight = 0.05;
        const waveSpeed = 2;
        const waveCount = 3;

        for (let i = 0; i < positions.length; i += 3) {
            const x = this.originalPositions[i];
            const y = this.originalPositions[i + 1];
            const dist = Math.sqrt(x * x + y * y) / this.radius;

            if (dist < 1) {
                const wave = Math.sin(dist * waveCount * Math.PI * 2 + this.time * waveSpeed) * waveHeight * (1 - dist);
                positions[i + 2] = wave;
            }
        }

        this.mesh.geometry.attributes.position.needsUpdate = true;
        this.mesh.geometry.computeVertexNormals();

        const pulse = 0.5 + 0.2 * Math.sin(this.time * 1.5);
        this.edgeMesh.material.opacity = 0.3 + pulse * 0.3;
    }
}
