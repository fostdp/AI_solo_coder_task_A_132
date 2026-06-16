use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorData {
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub clepsydra_id: String,
    pub water_level: f64,
    pub flow_rate: f64,
    pub water_temp: f64,
    pub humidity: f64,
    pub quality: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClepsydraConfig {
    pub clepsydra_id: String,
    pub name: String,
    pub max_level: f64,
    pub min_level: f64,
    pub standard_flow: f64,
    pub cross_section_area: f64,
    pub orifice_diameter: f64,
    pub flow_coefficient: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HydraulicMetrics {
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub clepsydra_id: String,
    pub theoretical_flow: f64,
    pub actual_flow: f64,
    pub flow_error: f64,
    pub evaporation_rate: f64,
    pub daily_error_seconds: f64,
    pub compensation_flow: f64,
    pub pid_kp: f64,
    pub pid_ki: f64,
    pub pid_kd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlertType {
    WaterLevelHigh,
    WaterLevelLow,
    DailyErrorExceed,
    TempAbnormal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlertLevel {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    pub id: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub clepsydra_id: String,
    pub alert_type: AlertType,
    pub alert_level: AlertLevel,
    pub message: String,
    pub value: f64,
    pub threshold: f64,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PidState {
    pub kp: f64,
    pub ki: f64,
    pub kd: f64,
    pub integral: f64,
    pub prev_error: f64,
    pub output_min: f64,
    pub output_max: f64,
}

impl PidState {
    pub fn new(kp: f64, ki: f64, kd: f64, output_min: f64, output_max: f64) -> Self {
        Self {
            kp,
            ki,
            kd,
            integral: 0.0,
            prev_error: 0.0,
            output_min,
            output_max,
        }
    }

    pub fn compute(&mut self, setpoint: f64, actual: f64, dt: f64) -> f64 {
        let error = setpoint - actual;
        self.integral += error * dt;
        let derivative = (error - self.prev_error) / dt;
        let output = self.kp * error + self.ki * self.integral + self.kd * derivative;
        self.prev_error = error;
        output.clamp(self.output_min, self.output_max)
    }

    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.prev_error = 0.0;
    }
}

impl AlertType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AlertType::WaterLevelHigh => "WATER_LEVEL_HIGH",
            AlertType::WaterLevelLow => "WATER_LEVEL_LOW",
            AlertType::DailyErrorExceed => "DAILY_ERROR_EXCEED",
            AlertType::TempAbnormal => "TEMP_ABNORMAL",
        }
    }
}

impl AlertLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            AlertLevel::Info => "INFO",
            AlertLevel::Warning => "WARNING",
            AlertLevel::Critical => "CRITICAL",
        }
    }
}
