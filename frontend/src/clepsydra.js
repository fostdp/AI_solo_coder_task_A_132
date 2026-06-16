import * as THREE from 'three';
import { WaterSurface } from './water.js';

export class ClepsydraScene {
    constructor(scene) {
        this.scene = scene;
        this.clepsydras = {};
        this.waterSurfaces = {};
        this.labels = {};
        this.labelSprites = {};

        this.clepsydraConfigs = [
            { id: 'KD1', name: '天上壶', position: new THREE.Vector3(-25, 0, -15), height: 15, radius: 5, maxLevel: 120, minLevel: 20 },
            { id: 'KD2', name: '夜漏壶', position: new THREE.Vector3(-10, 0, 0), height: 13, radius: 4.5, maxLevel: 100, minLevel: 15 },
            { id: 'KD3', name: '平水壶', position: new THREE.Vector3(10, 0, 0), height: 11, radius: 4, maxLevel: 80, minLevel: 10 },
            { id: 'KD4', name: '万分水', position: new THREE.Vector3(25, 0, -10), height: 9, radius: 3.5, maxLevel: 60, minLevel: 5 },
        ];

        this.init();
    }

    init() {
        this.createBase();
        this.createClepsydras();
        this.createPipes();
        this.createDecorations();
    }

    createBase() {
        const baseGeometry = new THREE.BoxGeometry(70, 2, 40);
        const baseMaterial = new THREE.MeshStandardMaterial({
            color: 0x5a4a3a,
            roughness: 0.8,
            metalness: 0.1,
        });
        const base = new THREE.Mesh(baseGeometry, baseMaterial);
        base.position.y = -1;
        base.receiveShadow = true;
        this.scene.add(base);

        const frameGeometry = new THREE.BoxGeometry(72, 0.5, 42);
        const frameMaterial = new THREE.MeshStandardMaterial({
            color: 0x8b7355,
            roughness: 0.6,
            metalness: 0.2,
        });
        const frame = new THREE.Mesh(frameGeometry, frameMaterial);
        frame.position.y = 0.25;
        frame.receiveShadow = true;
        this.scene.add(frame);

        const decorationMaterial = new THREE.MeshStandardMaterial({
            color: 0xc9a227,
            roughness: 0.4,
            metalness: 0.6,
        });

        for (let i = 0; i < 4; i++) {
            const pillarGeometry = new THREE.CylinderGeometry(0.3, 0.4, 30, 8);
            const pillar = new THREE.Mesh(pillarGeometry, decorationMaterial);
            const x = i < 2 ? -33 : 33;
            const z = i % 2 === 0 ? -18 : 18;
            pillar.position.set(x, 15, z);
            pillar.castShadow = true;
            this.scene.add(pillar);
        }

        const roofGeometry = new THREE.ConeGeometry(42, 8, 4);
        const roofMaterial = new THREE.MeshStandardMaterial({
            color: 0x6b5b4b,
            roughness: 0.7,
            metalness: 0.1,
        });
        const roof = new THREE.Mesh(roofGeometry, roofMaterial);
        roof.position.y = 32;
        roof.rotation.y = Math.PI / 4;
        roof.castShadow = true;
        this.scene.add(roof);
    }

    createClepsydras() {
        for (const config of this.clepsydraConfigs) {
            const group = new THREE.Group();

            const bodyGeometry = new THREE.CylinderGeometry(
                config.radius * 1.1,
                config.radius * 0.9,
                config.height,
                32
            );
            const bodyMaterial = new THREE.MeshStandardMaterial({
                color: 0x8b7355,
                roughness: 0.7,
                metalness: 0.1,
                side: THREE.DoubleSide,
            });
            const body = new THREE.Mesh(bodyGeometry, bodyMaterial);
            body.position.y = config.height / 2;
            body.castShadow = true;
            body.receiveShadow = true;
            group.add(body);

            const innerGeometry = new THREE.CylinderGeometry(
                config.radius * 0.95,
                config.radius * 0.8,
                config.height - 1,
                32
            );
            const innerMaterial = new THREE.MeshStandardMaterial({
                color: 0x4a3a2a,
                roughness: 0.9,
                metalness: 0.0,
                side: THREE.BackSide,
            });
            const inner = new THREE.Mesh(innerGeometry, innerMaterial);
            inner.position.y = config.height / 2;
            group.add(inner);

            const rimGeometry = new THREE.TorusGeometry(config.radius * 1.05, 0.2, 8, 32);
            const rimMaterial = new THREE.MeshStandardMaterial({
                color: 0xc9a227,
                roughness: 0.4,
                metalness: 0.7,
            });
            const rim = new THREE.Mesh(rimGeometry, rimMaterial);
            rim.position.y = config.height;
            rim.rotation.x = Math.PI / 2;
            rim.castShadow = true;
            group.add(rim);

            const baseRim = new THREE.Mesh(rimGeometry, rimMaterial);
            baseRim.position.y = 0.5;
            baseRim.rotation.x = Math.PI / 2;
            baseRim.scale.set(0.9, 0.9, 0.9);
            baseRim.castShadow = true;
            group.add(baseRim);

            const spoutGeometry = new THREE.CylinderGeometry(0.2, 0.15, 2, 12);
            const spoutMaterial = new THREE.MeshStandardMaterial({
                color: 0x666666,
                roughness: 0.5,
                metalness: 0.6,
            });
            const spout = new THREE.Mesh(spoutGeometry, spoutMaterial);
            spout.position.set(config.radius * 0.8, 1, 0);
            spout.rotation.z = -Math.PI / 3;
            spout.castShadow = true;
            group.add(spout);

            const waterSurface = new WaterSurface(config.radius * 0.9);
            waterSurface.mesh.position.y = config.height * 0.6;
            group.add(waterSurface.mesh);

            this.waterSurfaces[config.id] = {
                surface: waterSurface,
                baseY: 1,
                maxY: config.height - 1,
                config: config,
            };

            const scaleLineMaterial = new THREE.LineBasicMaterial({ color: 0xcccccc, transparent: true, opacity: 0.6 });
            for (let i = 0; i <= 10; i++) {
                const y = 1 + (config.height - 2) * (i / 10);
                const points = [];
                points.push(new THREE.Vector3(-config.radius * 0.95, y, 0));
                points.push(new THREE.Vector3(-config.radius * 0.85, y, 0));
                const lineGeometry = new THREE.BufferGeometry().setFromPoints(points);
                const line = new THREE.Line(lineGeometry, scaleLineMaterial);
                group.add(line);
            }

            const labelSprite = this.createLabel(config.name);
            labelSprite.position.set(0, config.height + 2, 0);
            group.add(labelSprite);
            this.labelSprites[config.id] = labelSprite;

            const waterLabel = this.createWaterLevelLabel(`${config.maxLevel}cm`);
            waterLabel.position.set(config.radius + 1.5, config.height * 0.6, 0);
            group.add(waterLabel);
            this.labels[config.id] = waterLabel;

            group.position.copy(config.position);
            this.scene.add(group);
            this.clepsydras[config.id] = group;
        }
    }

    createLabel(text) {
        const canvas = document.createElement('canvas');
        const context = canvas.getContext('2d');
        canvas.width = 256;
        canvas.height = 64;

        context.fillStyle = 'rgba(201, 162, 39, 0.9)';
        context.fillRect(0, 0, canvas.width, canvas.height);

        context.strokeStyle = '#fff';
        context.lineWidth = 2;
        context.strokeRect(1, 1, canvas.width - 2, canvas.height - 2);

        context.font = 'bold 24px Microsoft YaHei, SimSun';
        context.fillStyle = '#000';
        context.textAlign = 'center';
        context.textBaseline = 'middle';
        context.fillText(text, canvas.width / 2, canvas.height / 2);

        const texture = new THREE.CanvasTexture(canvas);
        const material = new THREE.SpriteMaterial({
            map: texture,
            transparent: true,
            depthWrite: false,
        });
        const sprite = new THREE.Sprite(material);
        sprite.scale.set(6, 1.5, 1);
        return sprite;
    }

    createWaterLevelLabel(text) {
        const canvas = document.createElement('canvas');
        const context = canvas.getContext('2d');
        canvas.width = 128;
        canvas.height = 40;

        context.fillStyle = 'rgba(30, 144, 255, 0.8)';
        context.beginPath();
        context.roundRect(0, 0, canvas.width, canvas.height, 5);
        context.fill();

        context.font = 'bold 16px Courier New';
        context.fillStyle = '#fff';
        context.textAlign = 'center';
        context.textBaseline = 'middle';
        context.fillText(text, canvas.width / 2, canvas.height / 2);

        const texture = new THREE.CanvasTexture(canvas);
        const material = new THREE.SpriteMaterial({
            map: texture,
            transparent: true,
            depthWrite: false,
        });
        const sprite = new THREE.Sprite(material);
        sprite.scale.set(3, 0.9, 1);
        return sprite;
    }

    updateWaterLabel(id, text) {
        const label = this.labels[id];
        if (!label) return;

        const canvas = document.createElement('canvas');
        const context = canvas.getContext('2d');
        canvas.width = 128;
        canvas.height = 40;

        context.fillStyle = 'rgba(30, 144, 255, 0.8)';
        context.beginPath();
        if (context.roundRect) {
            context.roundRect(0, 0, canvas.width, canvas.height, 5);
        } else {
            context.rect(0, 0, canvas.width, canvas.height);
        }
        context.fill();

        context.font = 'bold 16px Courier New';
        context.fillStyle = '#fff';
        context.textAlign = 'center';
        context.textBaseline = 'middle';
        context.fillText(text, canvas.width / 2, canvas.height / 2);

        label.material.map.image = canvas;
        label.material.map.needsUpdate = true;
    }

    createPipes() {
        const pipeMaterial = new THREE.MeshStandardMaterial({
            color: 0x555555,
            roughness: 0.5,
            metalness: 0.6,
        });

        const pipeConfigs = [
            { from: new THREE.Vector3(-25, 8, -10), to: new THREE.Vector3(-10, 11, 3) },
            { from: new THREE.Vector3(-10, 7, 4), to: new THREE.Vector3(10, 9, 4) },
            { from: new THREE.Vector3(10, 6, 3), to: new THREE.Vector3(25, 7, -5) },
        ];

        for (const pipeConfig of pipeConfigs) {
            const direction = new THREE.Vector3().subVectors(pipeConfig.to, pipeConfig.from);
            const length = direction.length();
            const pipeGeometry = new THREE.CylinderGeometry(0.2, 0.2, length, 12);
            const pipe = new THREE.Mesh(pipeGeometry, pipeMaterial);
            pipe.position.copy(pipeConfig.from).add(direction.multiplyScalar(0.5));
            pipe.lookAt(pipeConfig.to);
            pipe.rotateX(Math.PI / 2);
            pipe.castShadow = true;
            this.scene.add(pipe);
        }
    }

    createDecorations() {
        const dragonMaterial = new THREE.MeshStandardMaterial({
            color: 0xc9a227,
            roughness: 0.4,
            metalness: 0.6,
        });

        const dragonHeadGeometry = new THREE.SphereGeometry(0.8, 16, 16);
        const dragonHead = new THREE.Mesh(dragonHeadGeometry, dragonMaterial);
        dragonHead.position.set(25, 4.5, -10);
        dragonHead.scale.set(1, 0.8, 1.2);
        dragonHead.castShadow = true;
        this.scene.add(dragonHead);

        const eyeGeometry = new THREE.SphereGeometry(0.1, 8, 8);
        const eyeMaterial = new THREE.MeshBasicMaterial({ color: 0xff0000 });
        const leftEye = new THREE.Mesh(eyeGeometry, eyeMaterial);
        leftEye.position.set(25.4, 4.7, -9);
        this.scene.add(leftEye);

        const rightEye = new THREE.Mesh(eyeGeometry, eyeMaterial);
        rightEye.position.set(25.4, 4.7, -11);
        this.scene.add(rightEye);

        const plaqueGeometry = new THREE.BoxGeometry(12, 2.5, 0.5);
        const plaqueMaterial = new THREE.MeshStandardMaterial({
            color: 0x8b0000,
            roughness: 0.6,
            metalness: 0.2,
        });
        const plaque = new THREE.Mesh(plaqueGeometry, plaqueMaterial);
        plaque.position.set(0, 28, -19);
        this.scene.add(plaque);
    }

    updateWaterLevel(id, levelRatio) {
        const waterData = this.waterSurfaces[id];
        if (!waterData) return;

        const { surface, baseY, maxY, config } = waterData;
        const targetY = baseY + (maxY - baseY) * Math.max(0, Math.min(1, levelRatio));

        surface.mesh.position.y += (targetY - surface.mesh.position.y) * 0.1;

        if (this.labels[id]) {
            this.labels[id].position.y = surface.mesh.position.y;
            const actualLevel = config.minLevel + (config.maxLevel - config.minLevel) * levelRatio;
            this.updateWaterLabel(id, `${actualLevel.toFixed(1)}cm`);
        }
    }

    setLabelsVisible(visible) {
        for (const id in this.labelSprites) {
            this.labelSprites[id].visible = visible;
        }
        for (const id in this.labels) {
            this.labels[id].visible = visible;
        }
    }

    update(delta) {
        for (const id in this.waterSurfaces) {
            this.waterSurfaces[id].surface.update(delta);
        }
    }
}
