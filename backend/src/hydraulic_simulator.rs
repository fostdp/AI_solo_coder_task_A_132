use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use chrono::Utc;
use parking_lot::Mutex;
use tracing::{warn, debug};

use crate::dtu_receiver::ValidatedSensor;
use crate::hydraulic::HydraulicModel;
use crate::models::{ClepsydraConfig, HydraulicMetrics, SensorData};
use crate::config_loader::AppConfig;
use crate::metrics::{
    set_water_level, set_flow_rate, set_theoretical_flow,
    set_flow_error, set_evaporation, observe_processing,
};

#[derive(Debug, Clone)]
pub struct SimulatorOutput {
    pub sensor: SensorData,
    pub metrics: HydraulicMetrics,
    pub config: ClepsydraConfig,
    pub dt: f64,
}

pub struct HydraulicSimulator {
    config: Arc<AppConfig>,
    configs: Arc<Mutex<HashMap<String, ClepsydraConfig>>>,
    hydraulic_model: Arc<HydraulicModel>,
    last_update: Arc<Mutex<HashMap<String, chrono::DateTime<Utc>>>>,
    daily_error_map: Arc<Mutex<HashMap<String, f64>>>,
    rx: tokio::sync::mpsc::Receiver<ValidatedSensor>,
    tx: tokio::sync::mpsc::Sender<SimulatorOutput>,
}

impl HydraulicSimulator {
    pub fn new(
        config: Arc<AppConfig>,
        configs: Arc<Mutex<HashMap<String, ClepsydraConfig>>>,
        hydraulic_model: Arc<HydraulicModel>,
        rx: tokio::sync::mpsc::Receiver<ValidatedSensor>,
        tx: tokio::sync::mpsc::Sender<SimulatorOutput>,
    ) -> Self {
        Self {
            config,
            configs,
            hydraulic_model,
            last_update: Arc::new(Mutex::new(HashMap::new())),
            daily_error_map: Arc::new(Mutex::new(HashMap::new())),
            rx,
            tx,
        }
    }

    pub fn get_state_refs(
        &self,
    ) -> (
        Arc<Mutex<HashMap<String, chrono::DateTime<Utc>>>>,
        Arc<Mutex<HashMap<String, f64>>>,
    ) {
        (self.last_update.clone(), self.daily_error_map.clone())
    }

    pub async fn run(mut self) -> Result<()> {
        debug!("[SIM] 水力仿真器启动");

        while let Some(validated) = self.rx.recv().await {
            let t_start = std::time::Instant::now();
            let ValidatedSensor { sensor, received_at: _ } = validated;

            let config = {
                let cfg_map = self.configs.lock();
                match cfg_map.get(&sensor.clepsydra_id) {
                    Some(c) => c.clone(),
                    None => {
                        warn!("[SIM] 未知漏壶ID: {}", sensor.clepsydra_id);
                        continue;
                    }
                }
            };

            let dt = {
                let mut last = self.last_update.lock();
                let prev = last
                    .get(&sensor.clepsydra_id)
                    .copied()
                    .unwrap_or(Utc::now());
                let delta =
                    (Utc::now() - prev).num_milliseconds() as f64 / 1000.0;
                last.insert(sensor.clepsydra_id.clone(), Utc::now());
                delta.max(self.config.hydraulic.min_dt_seconds)
            };

            let theoretical_flow = self.hydraulic_model.calculate_theoretical_flow(
                sensor.water_level,
                &config,
                sensor.water_temp,
            );

            let evaporation_rate = self.hydraulic_model.calculate_evaporation_rate(
                sensor.water_temp,
                sensor.humidity,
                config.cross_section_area,
                sensor.quality,
                sensor.pressure,
            );

            let flow_error = self
                .hydraulic_model
                .calculate_flow_error(theoretical_flow, sensor.flow_rate);

            let daily_error = {
                let mut errors = self.daily_error_map.lock();
                let current = errors
                    .get(&sensor.clepsydra_id)
                    .copied()
                    .unwrap_or(0.0);
                let new_error = self
                    .hydraulic_model
                    .update_daily_error(current, flow_error, dt);
                errors.insert(sensor.clepsydra_id.clone(), new_error);
                new_error
            };

            let id = &sensor.clepsydra_id;
            set_water_level(id, sensor.water_level);
            set_flow_rate(id, sensor.flow_rate);
            set_theoretical_flow(id, theoretical_flow);
            set_flow_error(id, flow_error);
            set_evaporation(id, evaporation_rate);
            observe_processing("simulator", t_start.elapsed().as_secs_f64());

            let metrics = HydraulicMetrics {
                timestamp: Utc::now(),
                clepsydra_id: sensor.clepsydra_id.clone(),
                theoretical_flow,
                actual_flow: sensor.flow_rate,
                flow_error,
                evaporation_rate,
                daily_error_seconds: daily_error,
                compensation_flow: 0.0,
                pid_kp: 0.0,
                pid_ki: 0.0,
                pid_kd: 0.0,
            };

            let output = SimulatorOutput {
                sensor: sensor.clone(),
                metrics,
                config,
                dt,
            };

            if let Err(e) = self.tx.send(output).await {
                warn!("[SIM] 下游通道关闭: {}", e);
                break;
            }
        }

        debug!("[SIM] 水力仿真器退出");
        Ok(())
    }
}
