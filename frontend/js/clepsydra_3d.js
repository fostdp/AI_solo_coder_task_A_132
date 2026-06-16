import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { WaterParticles } from '../src/particles.js';
import { ClepsydraScene } from '../src/clepsydra.js';

export class Clepsydra3D {
    constructor(canvas) {
        this.canvas = canvas;
        this.autoRotate = true;
        this.showParticles = false;
        this.showLabels = true;

        this._initScene();
        this._initCamera();
        this._initRenderer();
        this._initControls();
        this._initLights();
        this._initGround();
        this._initClepsydra();

        this._lastTime = 0;
        this._running = false;

        window.addEventListener('resize', () => this._onResize());
    }

    _initScene() {
        this.scene = new THREE.Scene();
        this.scene.background = new THREE.Color(0x0a0a1a);
        this.scene.fog = new THREE.Fog(0x0a0a1a, 50, 200);
    }

    _initCamera() {
        this.camera = new THREE.PerspectiveCamera(
            60,
            this.canvas.clientWidth / this.canvas.clientHeight,
            0.1,
            1000
        );
        this.camera.position.set(40, 30, 50);
    }

    _initRenderer() {
        this.renderer = new THREE.WebGLRenderer({
            canvas: this.canvas,
            antialias: true,
        });
        this.renderer.setSize(this.canvas.clientWidth, this.canvas.clientHeight);
        this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
        this.renderer.shadowMap.enabled = true;
        this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    }

    _initControls() {
        this.controls = new OrbitControls(this.camera, this.renderer.domElement);
        this.controls.enableDamping = true;
        this.controls.dampingFactor = 0.05;
        this.controls.minDistance = 20;
        this.controls.maxDistance = 150;
        this.controls.maxPolarAngle = Math.PI / 2.1;
    }

    _initLights() {
        const ambient = new THREE.AmbientLight(0x404060, 0.5);
        this.scene.add(ambient);

        const main = new THREE.DirectionalLight(0xfff5e6, 1.2);
        main.position.set(30, 50, 30);
        main.castShadow = true;
        main.shadow.mapSize.width = 2048;
        main.shadow.mapSize.height = 2048;
        main.shadow.camera.near = 0.5;
        main.shadow.camera.far = 200;
        main.shadow.camera.left = -60;
        main.shadow.camera.right = 60;
        main.shadow.camera.top = 60;
        main.shadow.camera.bottom = -60;
        this.scene.add(main);

        const fill = new THREE.DirectionalLight(0x4488ff, 0.3);
        fill.position.set(-30, 20, -30);
        this.scene.add(fill);
    }

    _initGround() {
        const ground = new THREE.Mesh(
            new THREE.CircleGeometry(80, 64),
            new THREE.MeshStandardMaterial({
                color: 0x2a2a3a,
                roughness: 0.8,
                metalness: 0.2,
            })
        );
        ground.rotation.x = -Math.PI / 2;
        ground.receiveShadow = true;
        this.scene.add(ground);

        const grid = new THREE.GridHelper(80, 40, 0x333355, 0x222244);
        grid.position.y = 0.01;
        this.scene.add(grid);
    }

    _initClepsydra() {
        this.clepsydraScene = new ClepsydraScene(this.scene);
        this.waterParticles = new WaterParticles(this.scene);
        this.waterParticles.setVisible(false);
    }

    setAutoRotate(flag) {
        this.autoRotate = flag;
    }

    setShowParticles(flag) {
        this.showParticles = flag;
        this.waterParticles.setVisible(flag);
    }

    setLabelsVisible(flag) {
        this.showLabels = flag;
        this.clepsydraScene.setLabelsVisible(flag);
    }

    updateWaterLevel(id, levelRatio) {
        this.clepsydraScene.updateWaterLevel(id, levelRatio);
    }

    setParticleFlow(id, normalizedFlow) {
        if (!this.showParticles || normalizedFlow <= 0) return;
        const idx = ['KD1', 'KD2', 'KD3', 'KD4'].indexOf(id);
        if (idx >= 0 && idx < 3) {
            this.waterParticles.setFlowRate(idx, normalizedFlow);
        }
    }

    _onResize() {
        this.camera.aspect = this.canvas.clientWidth / this.canvas.clientHeight;
        this.camera.updateProjectionMatrix();
        this.renderer.setSize(this.canvas.clientWidth, this.canvas.clientHeight);
    }

    start() {
        if (this._running) return;
        this._running = true;
        this._animate(0);
    }

    _animate(currentTime) {
        if (!this._running) return;
        requestAnimationFrame((t) => this._animate(t));

        const delta = Math.min((currentTime - this._lastTime) / 1000, 0.05);
        this._lastTime = currentTime;

        if (this.autoRotate) {
            const angle = delta * 0.1;
            const radius = this.camera.position.length();
            this.camera.position.x =
                Math.cos(angle * Math.PI) * radius * 0.7 +
                this.camera.position.x * 0.3;
            this.camera.position.z =
                Math.sin(angle * Math.PI) * radius * 0.7 +
                this.camera.position.z * 0.3;
            this.controls.update();
        }

        this.waterParticles.update(delta);
        this.clepsydraScene.update(delta);

        this.renderer.render(this.scene, this.camera);
    }

    dispose() {
        this._running = false;
        this.waterParticles.dispose?.();
        this.renderer.dispose();
    }
}
