import * as THREE from 'three';

export class WaterParticles {
    constructor(scene) {
        this.scene = scene;
        this.particleSystems = [];
        this.flowRates = [1, 1, 1];
        this.visible = false;

        this.positions = [
            {
                start: new THREE.Vector3(-25 + 3.5, 8, -10),
                end: new THREE.Vector3(-10, 12, 3),
            },
            {
                start: new THREE.Vector3(-10 + 3.2, 7, 4),
                end: new THREE.Vector3(10, 10, 4),
            },
            {
                start: new THREE.Vector3(10 + 2.8, 6, 3),
                end: new THREE.Vector3(25, 8, -5),
            },
        ];

        this.init();
    }

    init() {
        for (let i = 0; i < this.positions.length; i++) {
            const system = this.createParticleSystem(i);
            this.particleSystems.push(system);
            this.scene.add(system.mesh);
            system.mesh.visible = false;
        }
    }

    createParticleSystem(index) {
        const particleCount = 500;
        const geometry = new THREE.BufferGeometry();
        const positions = new Float32Array(particleCount * 3);
        const velocities = new Float32Array(particleCount * 3);
        const lifetimes = new Float32Array(particleCount);
        const maxLifetimes = new Float32Array(particleCount);
        const sizes = new Float32Array(particleCount);

        const pos = this.positions[index];

        for (let i = 0; i < particleCount; i++) {
            this.resetParticle(
                positions, velocities, lifetimes, maxLifetimes, sizes,
                i, pos.start, pos.end
            );
        }

        geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
        geometry.setAttribute('size', new THREE.BufferAttribute(sizes, 1));

        const canvas = document.createElement('canvas');
        canvas.width = 64;
        canvas.height = 64;
        const ctx = canvas.getContext('2d');

        const gradient = ctx.createRadialGradient(32, 32, 0, 32, 32, 32);
        gradient.addColorStop(0, 'rgba(100, 200, 255, 1)');
        gradient.addColorStop(0.3, 'rgba(50, 150, 255, 0.8)');
        gradient.addColorStop(0.6, 'rgba(30, 100, 255, 0.4)');
        gradient.addColorStop(1, 'rgba(0, 50, 200, 0)');

        ctx.fillStyle = gradient;
        ctx.fillRect(0, 0, 64, 64);

        const texture = new THREE.CanvasTexture(canvas);

        const material = new THREE.PointsMaterial({
            size: 0.3,
            map: texture,
            transparent: true,
            opacity: 0.8,
            depthWrite: false,
            blending: THREE.AdditiveBlending,
            sizeAttenuation: true,
        });

        const mesh = new THREE.Points(geometry, material);

        return {
            mesh,
            positions,
            velocities,
            lifetimes,
            maxLifetimes,
            sizes,
            particleCount,
            pos,
            index,
        };
    }

    resetParticle(positions, velocities, lifetimes, maxLifetimes, sizes, index, start, end) {
        const i3 = index * 3;
        const offset = (Math.random() - 0.5) * 0.5;

        positions[i3] = start.x + (Math.random() - 0.5) * 0.3;
        positions[i3 + 1] = start.y + offset;
        positions[i3 + 2] = start.z + (Math.random() - 0.5) * 0.3;

        const direction = new THREE.Vector3()
            .subVectors(end, start)
            .normalize();

        const speed = 3 + Math.random() * 2;
        velocities[i3] = direction.x * speed + (Math.random() - 0.5) * 0.5;
        velocities[i3 + 1] = direction.y * speed - 2;
        velocities[i3 + 2] = direction.z * speed + (Math.random() - 0.5) * 0.5;

        lifetimes[index] = 0;
        maxLifetimes[index] = 0.8 + Math.random() * 0.4;
        sizes[index] = 0.2 + Math.random() * 0.2;
    }

    setFlowRate(index, rate) {
        if (index >= 0 && index < this.flowRates.length) {
            this.flowRates[index] = Math.max(0, Math.min(2, rate));
        }
    }

    setVisible(visible) {
        this.visible = visible;
        for (const system of this.particleSystems) {
            system.mesh.visible = visible;
        }
    }

    update(delta) {
        if (!this.visible) return;

        for (const system of this.particleSystems) {
            const flowRate = this.flowRates[system.index] || 1;
            const { positions, velocities, lifetimes, maxLifetimes, sizes, particleCount, pos } = system;

            for (let i = 0; i < particleCount; i++) {
                const i3 = i * 3;

                if (Math.random() < flowRate * 0.1) {
                    lifetimes[i] += delta * flowRate;
                }

                if (lifetimes[i] >= maxLifetimes[i]) {
                    this.resetParticle(
                        positions, velocities, lifetimes, maxLifetimes, sizes,
                        i, pos.start, pos.end
                    );
                    continue;
                }

                positions[i3] += velocities[i3] * delta * flowRate;
                positions[i3 + 1] += velocities[i3 + 1] * delta * flowRate - 0.5 * 9.8 * delta * delta;
                positions[i3 + 2] += velocities[i3 + 2] * delta * flowRate;

                velocities[i3 + 1] -= 9.8 * delta * 0.5;

                const lifeRatio = lifetimes[i] / maxLifetimes[i];
                sizes[i] = (0.2 + lifeRatio * 0.1) * flowRate;
            }

            system.mesh.geometry.attributes.position.needsUpdate = true;
            system.mesh.material.opacity = 0.6 * flowRate;
        }
    }
}
