use std::sync::Arc;
use anyhow::{Result, Context};
use tracing::{info, error, warn, debug};
use uuid::Uuid;

use crate::mqtt_receiver::MqttReceiver;
use crate::models::SensorData;
use crate::config_loader::{AppConfig, MqttConfig};
use crate::metrics::{record_sensor_received, record_validation_error};

#[derive(Debug, Clone)]
pub struct ValidatedSensor {
    pub sensor: SensorData,
    pub received_at: chrono::DateTime<chrono::Utc>,
}

fn validate_sensor(sensor: &SensorData, cfg: &AppConfig) -> Result<()> {
    if sensor.clepsydra_id.trim().is_empty() {
        record_validation_error("unknown", "empty_id");
        anyhow::bail!("空的clepsydra_id");
    }
    if !cfg.clepsydras.iter().any(|c| c.clepsydra_id == sensor.clepsydra_id) {
        record_validation_error(&sensor.clepsydra_id, "unknown_id");
        anyhow::bail!("未知漏壶ID: {}", sensor.clepsydra_id);
    }
    if sensor.water_level < 0.0 || sensor.water_level > 500.0 {
        record_validation_error(&sensor.clepsydra_id, "water_level_out_of_range");
        anyhow::bail!("水位超范围: {:.2}cm", sensor.water_level);
    }
    if sensor.flow_rate < -10.0 || sensor.flow_rate > 100.0 {
        record_validation_error(&sensor.clepsydra_id, "flow_rate_out_of_range");
        anyhow::bail!("流量超范围: {:.2}mL/s", sensor.flow_rate);
    }
    if sensor.water_temp < -50.0 || sensor.water_temp > 100.0 {
        record_validation_error(&sensor.clepsydra_id, "temp_out_of_range");
        anyhow::bail!("水温超范围: {:.2}°C", sensor.water_temp);
    }
    if sensor.humidity < 0.0 || sensor.humidity > 100.0 {
        record_validation_error(&sensor.clepsydra_id, "humidity_out_of_range");
        anyhow::bail!("湿度超范围: {:.2}%", sensor.humidity);
    }
    if sensor.quality < 0.0 || sensor.quality > 2.0 {
        record_validation_error(&sensor.clepsydra_id, "quality_out_of_range");
        anyhow::bail!("水质系数超范围: {:.2}", sensor.quality);
    }
    if sensor.pressure < 50.0 || sensor.pressure > 150.0 {
        record_validation_error(&sensor.clepsydra_id, "pressure_out_of_range");
        anyhow::bail!("气压超范围: {:.2}kPa", sensor.pressure);
    }
    Ok(())
}

pub struct DtuReceiver {
    config: Arc<AppConfig>,
    tx: tokio::sync::mpsc::Sender<ValidatedSensor>,
}

impl DtuReceiver {
    pub fn new(config: Arc<AppConfig>, tx: tokio::sync::mpsc::Sender<ValidatedSensor>) -> Self {
        Self { config, tx }
    }

    pub async fn run(self) -> Result<()> {
        info!("[DTU] 启动MQTT接收端");
        let mqtt_cfg = &self.config.mqtt;
        let client_id = format!("{}-{}", mqtt_cfg.client_id_prefix, Uuid::new_v4());

        let mut receiver = MqttReceiver::new(
            &mqtt_cfg.broker,
            mqtt_cfg.port,
            &client_id,
            &mqtt_cfg.topic,
        ).context("创建MQTT接收器失败")?;

        receiver.subscribe().await.context("订阅MQTT主题失败")?;

        let config = self.config.clone();
        let tx = self.tx.clone();

        receiver.run(move |sensor_data: SensorData| {
            let config = config.clone();
            let tx = tx.clone();

            tokio::spawn(async move {
                let received_at = chrono::Utc::now();
                match validate_sensor(&sensor_data, &config) {
                    Ok(()) => {
                        let id = sensor_data.clepsydra_id.clone();
                        record_sensor_received(&id);
                        let validated = ValidatedSensor {
                            sensor: sensor_data,
                            received_at,
                        };
                        if let Err(e) = tx.send(validated).await {
                            warn!("[DTU] 下游通道已满或已关闭: {}", e);
                        } else {
                            debug!("[DTU] {} 校验通过，送入仿真管线", id);
                        }
                    }
                    Err(e) => {
                        warn!("[DTU] 传感器数据校验失败: {}", e);
                    }
                }
            });
        }).await?;

        Ok(())
    }
}

pub fn build_mqtt_config_from_env() -> MqttConfig {
    MqttConfig {
        broker: std::env::var("MQTT_BROKER")
            .unwrap_or_else(|_| "localhost".to_string()),
        port: std::env::var("MQTT_PORT")
            .unwrap_or_else(|_| "1883".to_string())
            .parse()
            .unwrap_or(1883),
        topic: std::env::var("MQTT_TOPIC")
            .unwrap_or_else(|_| "clepsydra/sensor/+".to_string()),
        client_id_prefix: std::env::var("MQTT_CLIENT_PREFIX")
            .unwrap_or_else(|_| "clepsydra-backend".to_string()),
        keep_alive_secs: std::env::var("MQTT_KEEP_ALIVE")
            .unwrap_or_else(|_| "30".to_string())
            .parse()
            .unwrap_or(30),
    }
}
