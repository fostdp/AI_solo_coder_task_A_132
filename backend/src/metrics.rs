use std::sync::Arc;
use once_cell::sync::Lazy;
use prometheus::{
    register_counter_vec, register_gauge_vec, register_histogram_vec,
    CounterVec, GaugeVec, HistogramVec, Encoder, TextEncoder,
};
use tracing::warn;

pub static REGISTRY: Lazy<prometheus::Registry> = Lazy::new(|| {
    prometheus::Registry::new()
});

pub static SENSOR_RECEIVED_TOTAL: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        "clepsydra_sensor_received_total",
        "Total number of sensor data points received",
        &["clepsydra_id"]
    )
    .unwrap()
});

pub static SENSOR_VALIDATION_ERRORS: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        "clepsydra_sensor_validation_errors_total",
        "Total number of sensor validation errors",
        &["clepsydra_id", "error_type"]
    )
    .unwrap()
});

pub static WATER_LEVEL_GAUGE: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec!(
        "clepsydra_water_level_cm",
        "Current water level in cm",
        &["clepsydra_id"]
    )
    .unwrap()
});

pub static FLOW_RATE_GAUGE: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec!(
        "clepsydra_flow_rate_ml_per_s",
        "Current flow rate in mL/s",
        &["clepsydra_id"]
    )
    .unwrap()
});

pub static THEORETICAL_FLOW_GAUGE: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec!(
        "clepsydra_theoretical_flow_ml_per_s",
        "Theoretical flow rate in mL/s",
        &["clepsydra_id"]
    )
    .unwrap()
});

pub static FLOW_ERROR_GAUGE: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec!(
        "clepsydra_flow_error_percent",
        "Flow error percentage",
        &["clepsydra_id"]
    )
    .unwrap()
});

pub static EVAPORATION_GAUGE: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec!(
        "clepsydra_evaporation_ml_per_s",
        "Evaporation rate in mL/s",
        &["clepsydra_id"]
    )
    .unwrap()
});

pub static DAILY_ERROR_GAUGE: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec!(
        "clepsydra_daily_error_seconds",
        "Daily timing error in seconds",
        &["clepsydra_id"]
    )
    .unwrap()
});

pub static COMPENSATION_FLOW_GAUGE: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec!(
        "clepsydra_compensation_flow_ml_per_s",
        "PID compensation flow in mL/s",
        &["clepsydra_id"]
    )
    .unwrap()
});

pub static ALERTS_TOTAL: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        "clepsydra_alerts_total",
        "Total number of alerts triggered",
        &["clepsydra_id", "alert_type", "alert_level"]
    )
    .unwrap()
});

pub static WS_CLIENTS_GAUGE: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec!(
        "clepsydra_ws_clients",
        "Number of connected WebSocket clients",
        &[]
    )
    .unwrap()
});

pub static PROCESSING_DURATION_HISTOGRAM: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "clepsydra_processing_duration_seconds",
        "Duration of sensor data processing pipeline",
        &["stage"],
        vec![0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1]
    )
    .unwrap()
});

pub fn init_metrics() -> Arc<prometheus::Registry> {
    let registry = &*REGISTRY;
    let r = registry.clone();

    macro_rules! register {
        ($metric:expr) => {
            let _ = r.register(Box::new($metric.clone()));
        };
    }

    register!(SENSOR_RECEIVED_TOTAL);
    register!(SENSOR_VALIDATION_ERRORS);
    register!(WATER_LEVEL_GAUGE);
    register!(FLOW_RATE_GAUGE);
    register!(THEORETICAL_FLOW_GAUGE);
    register!(FLOW_ERROR_GAUGE);
    register!(EVAPORATION_GAUGE);
    register!(DAILY_ERROR_GAUGE);
    register!(COMPENSATION_FLOW_GAUGE);
    register!(ALERTS_TOTAL);
    register!(WS_CLIENTS_GAUGE);
    register!(PROCESSING_DURATION_HISTOGRAM);

    Arc::new(r)
}

pub fn gather_metrics_text() -> String {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    match encoder.encode(&metric_families, &mut buffer) {
        Ok(()) => String::from_utf8_lossy(&buffer).into_owned(),
        Err(e) => {
            warn!("Failed to encode metrics: {}", e);
            String::new()
        }
    }
}

pub fn record_sensor_received(id: &str) {
    SENSOR_RECEIVED_TOTAL.with_label_values(&[id]).inc();
}

pub fn record_validation_error(id: &str, err_type: &str) {
    SENSOR_VALIDATION_ERRORS.with_label_values(&[id, err_type]).inc();
}

pub fn set_water_level(id: &str, value: f64) {
    WATER_LEVEL_GAUGE.with_label_values(&[id]).set(value);
}

pub fn set_flow_rate(id: &str, value: f64) {
    FLOW_RATE_GAUGE.with_label_values(&[id]).set(value);
}

pub fn set_theoretical_flow(id: &str, value: f64) {
    THEORETICAL_FLOW_GAUGE.with_label_values(&[id]).set(value);
}

pub fn set_flow_error(id: &str, value: f64) {
    FLOW_ERROR_GAUGE.with_label_values(&[id]).set(value);
}

pub fn set_evaporation(id: &str, value: f64) {
    EVAPORATION_GAUGE.with_label_values(&[id]).set(value);
}

pub fn set_daily_error(id: &str, value: f64) {
    DAILY_ERROR_GAUGE.with_label_values(&[id]).set(value);
}

pub fn set_compensation_flow(id: &str, value: f64) {
    COMPENSATION_FLOW_GAUGE.with_label_values(&[id]).set(value);
}

pub fn inc_alert(id: &str, alert_type: &str, level: &str) {
    ALERTS_TOTAL.with_label_values(&[id, alert_type, level]).inc();
}

pub fn set_ws_clients(count: f64) {
    WS_CLIENTS_GAUGE.with_label_values(&[]).set(count);
}

pub fn observe_processing(stage: &str, duration_secs: f64) {
    PROCESSING_DURATION_HISTOGRAM
        .with_label_values(&[stage])
        .observe(duration_secs);
}
