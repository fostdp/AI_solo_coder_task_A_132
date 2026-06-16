use serde::{Deserialize, Serialize};
use std::path::Path;
use std::collections::HashMap;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub mqtt: MqttConfig,
    pub clickhouse: ClickHouseConfig,
    pub server: ServerConfig,
    pub hydraulic: HydraulicConfig,
    pub pid: PidConfig,
    pub alerts: AlertConfig,
    pub clepsydras: Vec<ClepsydraEntry>,
    pub channels: ChannelConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttConfig {
    pub broker: String,
    pub port: u16,
    pub topic: String,
    pub client_id_prefix: String,
    pub keep_alive_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickHouseConfig {
    pub url: String,
    pub database: String,
    pub batch_size: usize,
    pub flush_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub cors_origins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HydraulicConfig {
    pub gravity_cm_s2: f64,
    pub standard_pressure_kpa: f64,
    pub min_dt_seconds: f64,
    pub altitude_m: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PidConfig {
    pub base_kp: f64,
    pub base_ki: f64,
    pub base_kd: f64,
    pub kf_feedforward: f64,
    pub output_min_ml_s: f64,
    pub output_max_ml_s: f64,
    pub output_rate_limit_ml_s2: f64,
    pub integral_limit: f64,
    pub temp_coefficient_per_deg: f64,
    pub history_window: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    pub daily_error_threshold_seconds: f64,
    pub critical_error_multiplier: f64,
    pub water_temp_min_c: f64,
    pub water_temp_max_c: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClepsydraEntry {
    pub clepsydra_id: String,
    pub name: String,
    pub max_level_cm: f64,
    pub min_level_cm: f64,
    pub standard_flow_ml_s: f64,
    pub cross_section_cm2: f64,
    pub orifice_diameter_cm: f64,
    pub flow_coefficient: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub dtu_to_simulator_buffer: usize,
    pub simulator_to_compensator_buffer: usize,
    pub compensator_to_alarm_buffer: usize,
    pub alarm_broadcast_capacity: usize,
}

impl AppConfig {
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let cfg: AppConfig = serde_json::from_str(&content)?;
        Ok(cfg)
    }

    pub fn to_clepsydra_map(&self) -> HashMap<String, crate::models::ClepsydraConfig> {
        let mut map = HashMap::new();
        for entry in &self.clepsydras {
            map.insert(entry.clepsydra_id.clone(), crate::models::ClepsydraConfig {
                clepsydra_id: entry.clepsydra_id.clone(),
                name: entry.name.clone(),
                max_level: entry.max_level_cm,
                min_level: entry.min_level_cm,
                standard_flow: entry.standard_flow_ml_s,
                cross_section_area: entry.cross_section_cm2,
                orifice_diameter: entry.orifice_diameter_cm,
                flow_coefficient: entry.flow_coefficient,
            });
        }
        map
    }
}
