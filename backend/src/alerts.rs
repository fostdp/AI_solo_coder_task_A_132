use std::collections::HashMap;
use parking_lot::Mutex;
use crate::models::{AlertEvent, AlertType, AlertLevel, SensorData, HydraulicMetrics, ClepsydraConfig};
use chrono::Utc;
use uuid::Uuid;

pub struct AlertManager {
    active_alerts: Mutex<HashMap<String, Vec<AlertEvent>>>,
    daily_error_threshold: f64,
}

impl AlertManager {
    pub fn new(daily_error_threshold_seconds: f64) -> Self {
        Self {
            active_alerts: Mutex::new(HashMap::new()),
            daily_error_threshold: daily_error_threshold_seconds,
        }
    }

    pub fn check_water_level(&self, sensor: &SensorData, config: &ClepsydraConfig) -> Option<AlertEvent> {
        let mut alerts = self.active_alerts.lock();

        if sensor.water_level > config.max_level {
            let alert = AlertEvent {
                id: Uuid::new_v4().to_string(),
                timestamp: Utc::now(),
                clepsydra_id: sensor.clepsydra_id.clone(),
                alert_type: AlertType::WaterLevelHigh,
                alert_level: AlertLevel::Critical,
                message: format!("漏壶{}水位过高: {:.2}cm > {:.2}cm",
                    config.name, sensor.water_level, config.max_level),
                value: sensor.water_level,
                threshold: config.max_level,
                resolved: false,
            };
            alerts.entry(sensor.clepsydra_id.clone())
                .or_default()
                .push(alert.clone());
            return Some(alert);
        }

        if sensor.water_level < config.min_level {
            let alert = AlertEvent {
                id: Uuid::new_v4().to_string(),
                timestamp: Utc::now(),
                clepsydra_id: sensor.clepsydra_id.clone(),
                alert_type: AlertType::WaterLevelLow,
                alert_level: AlertLevel::Critical,
                message: format!("漏壶{}水位过低: {:.2}cm < {:.2}cm",
                    config.name, sensor.water_level, config.min_level),
                value: sensor.water_level,
                threshold: config.min_level,
                resolved: false,
            };
            alerts.entry(sensor.clepsydra_id.clone())
                .or_default()
                .push(alert.clone());
            return Some(alert);
        }

        None
    }

    pub fn check_daily_error(&self, metrics: &HydraulicMetrics) -> Option<AlertEvent> {
        let error_abs = metrics.daily_error_seconds.abs();

        if error_abs > self.daily_error_threshold {
            let alert = AlertEvent {
                id: Uuid::new_v4().to_string(),
                timestamp: Utc::now(),
                clepsydra_id: metrics.clepsydra_id.clone(),
                alert_type: AlertType::DailyErrorExceed,
                alert_level: if error_abs > self.daily_error_threshold * 2.0 {
                    AlertLevel::Critical
                } else {
                    AlertLevel::Warning
                },
                message: format!("漏壶{}日误差超限: {:.2}秒 > {:.2}秒",
                    metrics.clepsydra_id, metrics.daily_error_seconds, self.daily_error_threshold),
                value: metrics.daily_error_seconds,
                threshold: self.daily_error_threshold,
                resolved: false,
            };

            let mut alerts = self.active_alerts.lock();
            alerts.entry(metrics.clepsydra_id.clone())
                .or_default()
                .push(alert.clone());
            return Some(alert);
        }

        None
    }

    pub fn check_temperature(&self, sensor: &SensorData) -> Option<AlertEvent> {
        if sensor.water_temp < 0.0 || sensor.water_temp > 50.0 {
            let alert = AlertEvent {
                id: Uuid::new_v4().to_string(),
                timestamp: Utc::now(),
                clepsydra_id: sensor.clepsydra_id.clone(),
                alert_type: AlertType::TempAbnormal,
                alert_level: AlertLevel::Warning,
                message: format!("漏壶{}水温异常: {:.2}°C",
                    sensor.clepsydra_id, sensor.water_temp),
                value: sensor.water_temp,
                threshold: 50.0,
                resolved: false,
            };

            let mut alerts = self.active_alerts.lock();
            alerts.entry(sensor.clepsydra_id.clone())
                .or_default()
                .push(alert.clone());
            return Some(alert);
        }

        None
    }

    pub fn get_active_alerts(&self, clepsydra_id: &str) -> Vec<AlertEvent> {
        let alerts = self.active_alerts.lock();
        alerts.get(clepsydra_id)
            .map(|v| v.iter().filter(|a| !a.resolved).cloned().collect())
            .unwrap_or_default()
    }

    pub fn resolve_alert(&self, alert_id: &str) -> bool {
        let mut alerts = self.active_alerts.lock();
        for (_, list) in alerts.iter_mut() {
            for alert in list.iter_mut() {
                if alert.id == alert_id {
                    alert.resolved = true;
                    return true;
                }
            }
        }
        false
    }
}

pub struct CompensationController {
    pid_state: Mutex<HashMap<String, crate::models::PidState>>,
}

impl CompensationController {
    pub fn new() -> Self {
        Self {
            pid_state: Mutex::new(HashMap::new()),
        }
    }

    pub fn get_or_create_pid(&self, clepsydra_id: &str) -> crate::models::PidState {
        let mut states = self.pid_state.lock();
        states.entry(clepsydra_id.to_string())
            .or_insert_with(|| crate::models::PidState::new(0.5, 0.1, 0.05, -1.0, 1.0))
            .clone()
    }

    pub fn compute_compensation(
        &self,
        clepsydra_id: &str,
        setpoint_flow: f64,
        actual_flow: f64,
        water_temp: f64,
        quality: f64,
        dt: f64,
    ) -> (f64, crate::models::PidState) {
        let mut states = self.pid_state.lock();
        let pid = states.entry(clepsydra_id.to_string())
            .or_insert_with(|| {
                let base_kp = 0.5;
                let temp_factor = 1.0 + (water_temp - 20.0) * 0.01;
                let quality_factor = quality;
                crate::models::PidState::new(
                    base_kp * temp_factor * quality_factor,
                    0.05 * temp_factor,
                    0.1,
                    -1.5,
                    1.5,
                )
                .with_feedforward(0.08)
                .with_rate_limit(0.3)
            });

        let compensation = pid.compute(setpoint_flow, actual_flow, water_temp, dt);
        (compensation, pid.clone())
    }

    pub fn reset_pid(&self, clepsydra_id: &str) {
        let mut states = self.pid_state.lock();
        if let Some(pid) = states.get_mut(clepsydra_id) {
            pid.reset();
        }
    }

    pub fn update_pid_params(&self, clepsydra_id: &str, kp: f64, ki: f64, kd: f64) {
        let mut states = self.pid_state.lock();
        if let Some(pid) = states.get_mut(clepsydra_id) {
            pid.kp = kp;
            pid.ki = ki;
            pid.kd = kd;
        }
    }
}

impl Default for CompensationController {
    fn default() -> Self {
        Self::new()
    }
}
