export class PrecisionPanel {
    constructor() {
        this.clepsydraData = {
            KD1: { name: '天上壶', water_level: 100, flow_rate: 2.5, water_temp: 20, humidity: 60, quality: 1.0, max_level: 120, min_level: 20 },
            KD2: { name: '夜漏壶', water_level: 85, flow_rate: 2.5, water_temp: 20, humidity: 60, quality: 1.0, max_level: 100, min_level: 15 },
            KD3: { name: '平水壶', water_level: 65, flow_rate: 2.5, water_temp: 20, humidity: 60, quality: 1.0, max_level: 80, min_level: 10 },
            KD4: { name: '万分水', water_level: 50, flow_rate: 2.5, water_temp: 20, humidity: 60, quality: 1.0, max_level: 60, min_level: 5 },
        };

        this.metricsData = {
            KD1: { theoretical_flow: 2.5, actual_flow: 2.5, flow_error: 0, evaporation_rate: 0.01, daily_error_seconds: 0, compensation_flow: 0 },
        };

        this.alerts = [];
        this.ws = null;
        this.onSensorUpdate = null;
        this.onMetricsUpdate = null;
    }

    setCallbacks(callbacks) {
        this.onSensorUpdate = callbacks.onSensorUpdate || null;
        this.onMetricsUpdate = callbacks.onMetricsUpdate || null;
    }

    connectWebSocket() {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        let wsUrl = `${protocol}//${window.location.host}/ws`;

        try {
            this.ws = new WebSocket(wsUrl);
        } catch (e) {
            this.ws = new WebSocket('ws://localhost:8080/ws');
        }

        this.ws.onopen = () => {
            document.getElementById('statusDot').classList.add('connected');
            document.getElementById('statusText').textContent = '已连接';
        };

        this.ws.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                this._handleMessage(data);
            } catch (e) {
                console.error('解析WebSocket消息失败:', e);
            }
        };

        this.ws.onclose = () => {
            document.getElementById('statusDot').classList.remove('connected');
            document.getElementById('statusText').textContent = '连接断开';
            setTimeout(() => this.connectWebSocket(), 3000);
        };

        this.ws.onerror = () => {
            document.getElementById('statusDot').classList.remove('connected');
            document.getElementById('statusText').textContent = '连接错误';
        };
    }

    _handleMessage(data) {
        if (data.type === 'SensorData' && data.data) {
            const s = data.data;
            if (this.clepsydraData[s.clepsydra_id]) {
                Object.assign(this.clepsydraData[s.clepsydra_id], {
                    water_level: s.water_level,
                    flow_rate: s.flow_rate,
                    water_temp: s.water_temp,
                    humidity: s.humidity,
                    quality: s.quality,
                });

                if (this.onSensorUpdate) {
                    const cfg = this.clepsydraData[s.clepsydra_id];
                    const levelRatio = (s.water_level - cfg.min_level) / (cfg.max_level - cfg.min_level);
                    this.onSensorUpdate(s.clepsydra_id, levelRatio, s.flow_rate);
                }
            }
            this.updateUI();
        } else if (data.type === 'HydraulicMetrics' && data.data) {
            this.metricsData[data.data.clepsydra_id] = data.data;
            if (this.onMetricsUpdate) this.onMetricsUpdate(data.data);
            this.updateUI();
        } else if (data.type === 'Alert' && data.data) {
            this.alerts.push(data.data);
            if (this.alerts.length > 50) this.alerts.shift();
            this.updateAlerts();
            this.updateUI();
        }
    }

    updateUI() {
        this._renderClepsydraList();
        this._renderDailyError();
        this._renderMetrics();
    }

    _renderClepsydraList() {
        const list = document.getElementById('clepsydraList');
        list.innerHTML = '';

        for (const [id, data] of Object.entries(this.clepsydraData)) {
            const metrics = this.metricsData[id] || { daily_error_seconds: 0, compensation_flow: 0 };
            const levelPercent = ((data.water_level - data.min_level) / (data.max_level - data.min_level)) * 100;
            const hasAlert = this.alerts.some(a => a.clepsydra_id === id && !a.resolved);
            const criticalAlert = this.alerts.some(a => a.clepsydra_id === id && a.alert_level === 'CRITICAL' && !a.resolved);
            const cardClass = criticalAlert ? 'critical' : (hasAlert ? 'warning' : '');

            const card = document.createElement('div');
            card.className = `clepsydra-card ${cardClass}`;
            card.innerHTML = `
                <div class="clepsydra-name">${data.name} <span style="font-size: 11px; color: #888;">(${id})</span></div>
                <div class="data-grid">
                    <div class="data-item">
                        <span class="data-label">水位</span>
                        <span class="data-value">${data.water_level.toFixed(2)}cm</span>
                    </div>
                    <div class="data-item">
                        <span class="data-label">流量</span>
                        <span class="data-value">${data.flow_rate.toFixed(4)}mL/s</span>
                    </div>
                    <div class="data-item">
                        <span class="data-label">水温</span>
                        <span class="data-value">${data.water_temp.toFixed(1)}°C</span>
                    </div>
                    <div class="data-item">
                        <span class="data-label">湿度</span>
                        <span class="data-value">${data.humidity.toFixed(1)}%</span>
                    </div>
                </div>
                <div class="water-bar">
                    <div class="water-bar-fill" style="width: ${Math.min(100, Math.max(0, levelPercent))}%"></div>
                </div>
                <div style="display: flex; justify-content: space-between; margin-top: 8px; font-size: 11px;">
                    <span style="color: #888;">日误差: <span style="color: ${Math.abs(metrics.daily_error_seconds) > 60 ? '#ff6666' : '#66ff66'};">${metrics.daily_error_seconds.toFixed(2)}s</span></span>
                    <span class="compensation-badge ${metrics.compensation_flow >= 0 ? 'positive' : 'negative'}">
                        PID: ${metrics.compensation_flow >= 0 ? '+' : ''}${metrics.compensation_flow.toFixed(3)}
                    </span>
                </div>
            `;
            list.appendChild(card);
        }
    }

    _renderDailyError() {
        const total = Object.values(this.metricsData).reduce((sum, m) => sum + (m.daily_error_seconds || 0), 0);
        const el = document.getElementById('dailyErrorPanel');
        el.textContent = `${total.toFixed(2)} 秒`;
        el.style.color = Math.abs(total) > 60 ? '#ff6666' : '#66ff66';
    }

    _renderMetrics() {
        const main = this.metricsData['KD3'] || this.metricsData['KD1'] || {};
        document.getElementById('metricTheoretical').textContent = `${(main.theoretical_flow || 0).toFixed(4)} mL/s`;
        document.getElementById('metricActual').textContent = `${(main.actual_flow || 0).toFixed(4)} mL/s`;
        document.getElementById('metricError').textContent = `${(main.flow_error || 0).toFixed(3)} %`;
        document.getElementById('metricEvap').textContent = `${(main.evaporation_rate || 0).toFixed(5)} mL/s`;

        const comp = main.compensation_flow || 0;
        document.getElementById('metricComp').textContent = `${comp >= 0 ? '+' : ''}${comp.toFixed(4)} mL/s`;
        document.getElementById('metricComp').className = `metrics-value ${comp >= 0 ? 'good' : 'error'}`;
    }

    updateAlerts() {
        const panel = document.getElementById('alertPanel');
        if (this.alerts.length === 0) {
            panel.innerHTML = '<div style="color: #666; font-size: 12px; text-align: center; padding: 20px;">暂无告警</div>';
            return;
        }

        panel.innerHTML = '';
        const active = this.alerts.filter(a => !a.resolved).slice(-5).reverse();
        for (const alert of active) {
            const level = alert.alert_level?.toLowerCase() || 'warning';
            const item = document.createElement('div');
            item.className = `alert-item ${level}`;
            const time = new Date(alert.timestamp).toLocaleTimeString('zh-CN');
            item.innerHTML = `
                <div class="alert-time">${time} · ${alert.clepsydra_id}</div>
                <div class="alert-message">${alert.message}</div>
            `;
            panel.appendChild(item);
        }
    }

    startSimulationFallback() {
        setTimeout(() => {
            if (this.ws?.readyState !== WebSocket.OPEN) {
                for (let i = 0; i < 50; i++) {
                    setTimeout(() => {
                        if (this.onSensorUpdate) {
                            for (const [id, data] of Object.entries(this.clepsydraData)) {
                                const levelRatio = (data.water_level - data.min_level) / (data.max_level - data.min_level);
                                this.onSensorUpdate(id, levelRatio, data.flow_rate);
                            }
                        }
                    }, i * 100);
                }
            }
        }, 1000);
    }
}
