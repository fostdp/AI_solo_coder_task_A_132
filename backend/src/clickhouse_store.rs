use std::time::Duration;
use std::fmt;
use anyhow::{Context, Result};
use clickhouse::{Client, Row};
use serde::{Deserialize, Serialize};
use crate::models::{SensorData, HydraulicMetrics, AlertEvent, ClepsydraConfig};

#[derive(Clone)]
pub struct ClickHouseStore {
    client: Client,
}

impl fmt::Debug for ClickHouseStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClickHouseStore")
            .field("client", &"<ClickHouse Client>")
            .finish()
    }
}

#[derive(Debug, Row, Serialize, Deserialize)]
struct SensorRow {
    timestamp: i64,
    clepsydra_id: String,
    water_level: f64,
    flow_rate: f64,
    water_temp: f64,
    humidity: f64,
    quality: f64,
    pressure: f64,
}

#[derive(Debug, Row, Serialize, Deserialize)]
struct MetricRow {
    timestamp: i64,
    clepsydra_id: String,
    theoretical_flow: f64,
    actual_flow: f64,
    flow_error: f64,
    evaporation_rate: f64,
    daily_error_seconds: f64,
    compensation_flow: f64,
    pid_kp: f64,
    pid_ki: f64,
    pid_kd: f64,
}

#[derive(Debug, Row, Serialize, Deserialize)]
struct AlertRow {
    id: String,
    timestamp: i64,
    clepsydra_id: String,
    alert_type: String,
    alert_level: String,
    message: String,
    value: f64,
    threshold: f64,
    resolved: u8,
}

#[derive(Debug, Row, Serialize, Deserialize)]
struct ConfigRow {
    clepsydra_id: String,
    name: String,
    max_level: f64,
    min_level: f64,
    standard_flow: f64,
    cross_section_area: f64,
    orifice_diameter: f64,
    flow_coefficient: f64,
}

impl ClickHouseStore {
    pub fn new(url: &str, database: &str) -> Result<Self> {
        let client = Client::default()
            .with_url(url)
            .with_database(database);
        Ok(Self { client })
    }

    pub async fn insert_sensor_data(&self, data: &SensorData) -> Result<()> {
        let row = SensorRow {
            timestamp: data.timestamp.timestamp_millis(),
            clepsydra_id: data.clepsydra_id.clone(),
            water_level: data.water_level,
            flow_rate: data.flow_rate,
            water_temp: data.water_temp,
            humidity: data.humidity,
            quality: data.quality,
            pressure: data.pressure,
        };

        let mut insert = self.client.insert("sensor_data")?;
        insert.write(&row).await?;
        insert.end().await?;
        Ok(())
    }

    pub async fn insert_metrics(&self, metrics: &HydraulicMetrics) -> Result<()> {
        let row = MetricRow {
            timestamp: metrics.timestamp.timestamp_millis(),
            clepsydra_id: metrics.clepsydra_id.clone(),
            theoretical_flow: metrics.theoretical_flow,
            actual_flow: metrics.actual_flow,
            flow_error: metrics.flow_error,
            evaporation_rate: metrics.evaporation_rate,
            daily_error_seconds: metrics.daily_error_seconds,
            compensation_flow: metrics.compensation_flow,
            pid_kp: metrics.pid_kp,
            pid_ki: metrics.pid_ki,
            pid_kd: metrics.pid_kd,
        };

        let mut insert = self.client.insert("hydraulic_metrics")?;
        insert.write(&row).await?;
        insert.end().await?;
        Ok(())
    }

    pub async fn insert_alert(&self, alert: &AlertEvent) -> Result<()> {
        let row = AlertRow {
            id: alert.id.clone(),
            timestamp: alert.timestamp.timestamp_millis(),
            clepsydra_id: alert.clepsydra_id.clone(),
            alert_type: alert.alert_type.as_str().to_string(),
            alert_level: alert.alert_level.as_str().to_string(),
            message: alert.message.clone(),
            value: alert.value,
            threshold: alert.threshold,
            resolved: if alert.resolved { 1 } else { 0 },
        };

        let mut insert = self.client.insert("alerts")?;
        insert.write(&row).await?;
        insert.end().await?;
        Ok(())
    }

    pub async fn get_config(&self, clepsydra_id: &str) -> Result<Option<ClepsydraConfig>> {
        let query = format!(
            "SELECT clepsydra_id, name, max_level, min_level, standard_flow, \
             cross_section_area, orifice_diameter, flow_coefficient \
             FROM clepsydra_config WHERE clepsydra_id = '{}' ORDER BY updated_at DESC LIMIT 1",
            clepsydra_id
        );

        let rows: Vec<ConfigRow> = self.client.query(&query).fetch_all().await
            .with_context(|| format!("Failed to fetch config for {}", clepsydra_id))?;

        Ok(rows.into_iter().next().map(|r| ClepsydraConfig {
            clepsydra_id: r.clepsydra_id,
            name: r.name,
            max_level: r.max_level,
            min_level: r.min_level,
            standard_flow: r.standard_flow,
            cross_section_area: r.cross_section_area,
            orifice_diameter: r.orifice_diameter,
            flow_coefficient: r.flow_coefficient,
        }))
    }

    pub async fn get_all_configs(&self) -> Result<Vec<ClepsydraConfig>> {
        let query = "SELECT clepsydra_id, name, max_level, min_level, standard_flow, \
                     cross_section_area, orifice_diameter, flow_coefficient \
                     FROM clepsydra_config ORDER BY clepsydra_id";

        let rows: Vec<ConfigRow> = self.client.query(query).fetch_all().await?;

        Ok(rows.into_iter().map(|r| ClepsydraConfig {
            clepsydra_id: r.clepsydra_id,
            name: r.name,
            max_level: r.max_level,
            min_level: r.min_level,
            standard_flow: r.standard_flow,
            cross_section_area: r.cross_section_area,
            orifice_diameter: r.orifice_diameter,
            flow_coefficient: r.flow_coefficient,
        }).collect())
    }

    pub async fn get_recent_sensor_data(&self, clepsydra_id: &str, limit: usize) -> Result<Vec<SensorData>> {
        use chrono::{TimeZone, Utc};

        let query = format!(
            "SELECT timestamp, clepsydra_id, water_level, flow_rate, water_temp, humidity, quality, pressure \
             FROM sensor_data WHERE clepsydra_id = '{}' \
             ORDER BY timestamp DESC LIMIT {}",
            clepsydra_id, limit
        );

        let rows: Vec<SensorRow> = self.client.query(&query).fetch_all().await?;

        Ok(rows.into_iter().map(|r| SensorData {
            timestamp: Utc.timestamp_millis_opt(r.timestamp).unwrap(),
            clepsydra_id: r.clepsydra_id,
            water_level: r.water_level,
            flow_rate: r.flow_rate,
            water_temp: r.water_temp,
            humidity: r.humidity,
            quality: r.quality,
            pressure: r.pressure,
        }).collect())
    }

    pub async fn ping(&self) -> Result<()> {
        let _: Vec<u8> = self.client.query("SELECT 1").fetch_all().await?;
        Ok(())
    }
}
