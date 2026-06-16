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

        this._tempVec = new THREE.Vector3();
        this._sharedTexture = null;
        this._dirtyFlags = [];

        this.init();
    }

    getSharedTexture() {
        if (this._sharedTexture) return this._sharedTexture;

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

        this._sharedTexture = new THREE.CanvasTexture(canvas);
        return this._sharedTexture;
    }

    init() {
        for (let i = 0; i < this.positions.length; i++) {
            const system = this.createParticleSystem(i);
            this.particleSystems.push(system);
            this._dirtyFlags.push(false);
            this.scene.add(system.mesh);
            system.mesh.visible = false;
        }
    }

    createParticleSystem(index) {
        const particleCount = 300;
        const geometry = new THREE.BufferGeometry();
        const positions = new Float32Array(particleCount * 3);
        const velocities = new Float32Array(particleCount * 3);
        const lifetimes = new Float32Array(particleCount);
        const maxLifetimes = new Float32Array(particleCount);
        const sizes = new Float32Array(particleCount);
        const active = new Uint8Array(particleCount);

        const pos = this.positions[index];

        for (let i = 0; i < particleCount; i++) {
            this.resetParticle(
                positions, velocities, lifetimes, maxLifetimes, sizes, active,
                i, pos.start, pos.end
            );
            active[i] = 0;
        }

        const positionAttr = new THREE.BufferAttribute(positions, 3);
        positionAttr.setUsage(THREE.DynamicDrawUsage);
        geometry.setAttribute('position', positionAttr);

        const sizeAttr = new THREE.BufferAttribute(sizes, 1);
        sizeAttr.setUsage(THREE.DynamicDrawUsage);
        geometry.setAttribute('size', sizeAttr);

        const texture = this.getSharedTexture();

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
            geometry,
            positions,
            velocities,
            lifetimes,
            maxLifetimes,
            sizes,
            active,
            particleCount,
            pos,
            index,
            positionAttr,
            sizeAttr,
            freeList: this.buildFreeList(particleCount),
            freeCount: particleCount,
            spawnCounter: 0,
            lastOpacity: 0.8,
        };
    }

    buildFreeList(count) {
        const list = new Uint32Array(count);
        for (let i = 0; i < count; i++) {
            list[i] = count - 1 - i;
        }
        return list;
    }

    acquireParticle(system) {
        if (system.freeCount === 0) {
            let oldestIdx = -1;
            let oldestLife = Infinity;
            for (let i = 0; i < system.particleCount; i++) {
                if (system.active[i] && system.lifetimes[i] < oldestLife) {
                    oldestLife = system.lifetimes[i];
                    oldestIdx = i;
                }
            }
            if (oldestIdx >= 0) {
                this.resetParticle(
                    system.positions, system.velocities, system.lifetimes,
                    system.maxLifetimes, system.sizes, system.active,
                    oldestIdx, system.pos.start, system.pos.end
                );
                system.active[oldestIdx] = 1;
                return oldestIdx;
            }
            return -1;
        }

        system.freeCount--;
        const idx = system.freeList[system.freeCount];
        this.resetParticle(
            system.positions, system.velocities, system.lifetimes,
            system.maxLifetimes, system.sizes, system.active,
            idx, system.pos.start, system.pos.end
        );
        system.active[idx] = 1;
        return idx;
    }

    releaseParticle(system, idx) {
        if (!system.active[idx]) return;
        system.active[idx] = 0;
        system.positions[idx * 3 + 1] = -10000;
        system.freeList[system.freeCount] = idx;
        system.freeCount++;
    }

    resetParticle(positions, velocities, lifetimes, maxLifetimes, sizes, active, index, start, end) {
        const i3 = index * 3;
        const offset = (Math.random() - 0.5) * 0.5;

        positions[i3] = start.x + (Math.random() - 0.5) * 0.3;
        positions[i3 + 1] = start.y + offset;
        positions[i3 + 2] = start.z + (Math.random() - 0.5) * 0.3;

        this._tempVec.subVectors(end, start).normalize();

        const speed = 3 + Math.random() * 2;
        velocities[i3] = this._tempVec.x * speed + (Math.random() - 0.5) * 0.5;
        velocities[i3 + 1] = this._tempVec.y * speed - 2;
        velocities[i3 + 2] = this._tempVec.z * speed + (Math.random() - 0.5) * 0.5;

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
        for (let i = 0; i < this.particleSystems.length; i++) {
            const system = this.particleSystems[i];
            system.mesh.visible = visible;
            if (!visible) {
                for (let j = 0; j < system.particleCount; j++) {
                    if (system.active[j]) {
                        this.releaseParticle(system, j);
                    }
                }
                this._dirtyFlags[i] = true;
            }
        }
    }

    update(delta) {
        if (!this.visible) return;

        const clampedDelta = Math.min(delta, 0.05);

        for (let s = 0; s < this.particleSystems.length; s++) {
            const system = this.particleSystems[s];
            const flowRate = this.flowRates[system.index] || 1;

            let positionDirty = false;
            let sizeDirty = false;

            const spawnCount = Math.floor(flowRate * 80 * clampedDelta);
            system.spawnCounter += (flowRate * 80 * clampedDelta) - spawnCount;
            const extraSpawn = system.spawnCounter >= 1 ? Math.floor(system.spawnCounter) : 0;
            system.spawnCounter -= extraSpawn;

            for (let n = 0; n < spawnCount + extraSpawn; n++) {
                const idx = this.acquireParticle(system);
                if (idx >= 0) {
                    positionDirty = true;
                }
            }

            for (let i = 0; i < system.particleCount; i++) {
                if (!system.active[i]) continue;

                const i3 = i * 3;

                system.lifetimes[i] += clampedDelta * flowRate;

                if (system.lifetimes[i] >= system.maxLifetimes[i]) {
                    this.releaseParticle(system, i);
                    positionDirty = true;
                    continue;
                }

                system.positions[i3] += system.velocities[i3] * clampedDelta * flowRate;
                system.positions[i3 + 1] += system.velocities[i3 + 1] * clampedDelta * flowRate
                    - 0.5 * 9.8 * clampedDelta * clampedDelta;
                system.positions[i3 + 2] += system.velocities[i3 + 2] * clampedDelta * flowRate;

                system.velocities[i3 + 1] -= 9.8 * clampedDelta * 0.5;

                const lifeRatio = system.lifetimes[i] / system.maxLifetimes[i];
                const newSize = (0.2 + lifeRatio * 0.1) * flowRate;
                if (Math.abs(system.sizes[i] - newSize) > 0.001) {
                    system.sizes[i] = newSize;
                    sizeDirty = true;
                }

                if (system.positions[i3 + 1] < -20) {
                    this.releaseParticle(system, i);
                    positionDirty = true;
                } else {
                    positionDirty = true;
                }
            }

            if (positionDirty) {
                system.positionAttr.needsUpdate = true;
            }
            if (sizeDirty) {
                system.sizeAttr.needsUpdate = true;
            }

            const targetOpacity = 0.6 * flowRate;
            if (Math.abs(system.lastOpacity - targetOpacity) > 0.01) {
                system.mesh.material.opacity = targetOpacity;
                system.lastOpacity = targetOpacity;
            }
        }
    }

    dispose() {
        for (const system of this.particleSystems) {
            system.geometry.dispose();
            system.mesh.material.dispose();
            this.scene.remove(system.mesh);
        }
        if (this._sharedTexture) {
            this._sharedTexture.dispose();
            this._sharedTexture = null;
        }
        this.particleSystems.length = 0;
    }
}
