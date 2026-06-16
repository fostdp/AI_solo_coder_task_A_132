use std::time::Duration;
use anyhow::{Context, Result};
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS};
use serde::Deserialize;
use crate::models::SensorData;
use tracing::{info, warn, error};

#[derive(Debug, Deserialize)]
struct MqttSensorPayload {
    water_level: f64,
    flow_rate: f64,
    water_temp: f64,
    humidity: f64,
    quality: f64,
    #[serde(default = "default_pressure")]
    pressure: f64,
    #[serde(default)]
    timestamp: Option<i64>,
}

fn default_pressure() -> f64 {
    101.325
}

pub struct MqttReceiver {
    client: AsyncClient,
    eventloop: Option<EventLoop>,
    topic: String,
}

impl MqttReceiver {
    pub fn new(broker: &str, port: u16, client_id: &str, topic: &str) -> Result<Self> {
        let mut options = MqttOptions::new(client_id, broker, port);
        options.set_keep_alive(Duration::from_secs(30));
        options.set_clean_session(true);

        let (client, eventloop) = AsyncClient::new(options, 100);

        Ok(Self {
            client,
            eventloop: Some(eventloop),
            topic: topic.to_string(),
        })
    }

    pub async fn subscribe(&mut self) -> Result<()> {
        self.client.subscribe(&self.topic, QoS::AtLeastOnce)
            .await
            .context("Failed to subscribe to MQTT topic")?;
        info!("Subscribed to MQTT topic: {}", self.topic);
        Ok(())
    }

    pub async fn run<F>(mut self, mut callback: F) -> Result<()>
    where
        F: FnMut(SensorData) + Send + 'static,
    {
        let mut eventloop = self.eventloop.take()
            .context("EventLoop already taken")?;

        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::Publish(publish))) => {
                    match Self::parse_sensor_data(&publish.topic, &publish.payload) {
                        Ok(sensor_data) => {
                            callback(sensor_data);
                        }
                        Err(e) => {
                            warn!("Failed to parse sensor data: {}", e);
                        }
                    }
                }
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
                    info!("MQTT connected");
                    if let Err(e) = self.client.subscribe(&self.topic, QoS::AtLeastOnce).await {
                        error!("Failed to subscribe after reconnect: {}", e);
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    error!("MQTT error: {}", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    fn parse_sensor_data(topic: &str, payload: &[u8]) -> Result<SensorData> {
        let payload_str = std::str::from_utf8(payload)?;
        let parsed: MqttSensorPayload = serde_json::from_str(payload_str)?;

        let clepsydra_id = topic.split('/').last()
            .unwrap_or("unknown")
            .to_string();

        let timestamp = if let Some(ts) = parsed.timestamp {
            chrono::DateTime::from_timestamp_millis(ts)
                .unwrap_or_else(chrono::Utc::now)
        } else {
            chrono::Utc::now()
        };

        Ok(SensorData {
            timestamp,
            clepsydra_id,
            water_level: parsed.water_level,
            flow_rate: parsed.flow_rate,
            water_temp: parsed.water_temp,
            humidity: parsed.humidity,
            quality: parsed.quality,
            pressure: parsed.pressure,
        })
    }
}
