use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use parking_lot::Mutex;
use tracing::{debug, warn};

use crate::hydraulic_simulator::SimulatorOutput;
use crate::models::{HydraulicMetrics, PidState, SensorData, ClepsydraConfig};
use crate::config_loader::{AppConfig, PidConfig};
use crate::metrics::{set_daily_error, set_compensation_flow, observe_processing};

#[derive(Debug, Clone)]
pub struct CompensatedOutput {
    pub sensor: SensorData,
    pub metrics: HydraulicMetrics,
    pub config: ClepsydraConfig,
}

pub struct ErrorCompensator {
    config: Arc<AppConfig>,
    pid_states: Arc<Mutex<HashMap<String, PidState>>>,
    rx: tokio::sync::mpsc::Receiver<SimulatorOutput>,
    tx: tokio::sync::mpsc::Sender<CompensatedOutput>,
}

impl ErrorCompensator {
    pub fn new(
        config: Arc<AppConfig>,
        rx: tokio::sync::mpsc::Receiver<SimulatorOutput>,
        tx: tokio::sync::mpsc::Sender<CompensatedOutput>,
    ) -> Self {
        Self {
            config,
            pid_states: Arc::new(Mutex::new(HashMap::new())),
            rx,
            tx,
        }
    }

    pub fn get_pid_states_ref(&self) -> Arc<Mutex<HashMap<String, PidState>>> {
        self.pid_states.clone()
    }

    fn build_pid_state(pid_cfg: &PidConfig, water_temp: f64, quality: f64) -> PidState {
        let temp_factor = 1.0 + pid_cfg.temp_coefficient_per_deg * (water_temp - 20.0).abs();
        let quality_factor = 1.0 + (1.0 - quality).abs() * 0.5;

        let kp = pid_cfg.base_kp * temp_factor * quality_factor;
        let ki = pid_cfg.base_ki * temp_factor;
        let kd = pid_cfg.base_kd;

        let mut state = PidState::new(
            kp,
            ki,
            kd,
            pid_cfg.output_min_ml_s,
            pid_cfg.output_max_ml_s,
        );
        state = state.with_feedforward(pid_cfg.kf_feedforward);
        state = state.with_rate_limit(pid_cfg.output_rate_limit_ml_s2);
        state.integral_limit = pid_cfg.integral_limit;
        state
    }

    pub async fn run(mut self) -> Result<()> {
        debug!("[PID] 误差补偿器启动");

        while let Some(input) = self.rx.recv().await {
            let t_start = std::time::Instant::now();
            let SimulatorOutput { sensor, metrics, config, dt } = input;

            let compensation_flow = {
                let mut states = self.pid_states.lock();
                let pid_cfg = &self.config.pid;
                let state = states
                    .entry(sensor.clepsydra_id.clone())
                    .or_insert_with(|| {
                        Self::build_pid_state(
                            pid_cfg,
                            sensor.water_temp,
                            sensor.quality,
                        )
                    });

                let output = state.compute(
                    config.standard_flow,
                    sensor.flow_rate,
                    sensor.water_temp,
                    dt,
                );
                output
            };

            let id = sensor.clepsydra_id.clone();
            set_daily_error(&id, metrics.daily_error_seconds);
            set_compensation_flow(&id, compensation_flow);
            observe_processing("compensator", t_start.elapsed().as_secs_f64());

            let updated_metrics = HydraulicMetrics {
                compensation_flow,
                pid_kp: {
                    let states = self.pid_states.lock();
                    states.get(&sensor.clepsydra_id).map(|s| s.kp).unwrap_or(0.0)
                },
                pid_ki: {
                    let states = self.pid_states.lock();
                    states.get(&sensor.clepsydra_id).map(|s| s.ki).unwrap_or(0.0)
                },
                pid_kd: {
                    let states = self.pid_states.lock();
                    states.get(&sensor.clepsydra_id).map(|s| s.kd).unwrap_or(0.0)
                },
                ..metrics
            };

            let output = CompensatedOutput {
                sensor,
                metrics: updated_metrics,
                config,
            };

            if let Err(e) = self.tx.send(output).await {
                warn!("[PID] 下游通道关闭: {}", e);
                break;
            }
        }

        debug!("[PID] 误差补偿器退出");
        Ok(())
    }
}
