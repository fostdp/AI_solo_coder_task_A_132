mod models;
mod hydraulic;
mod clickhouse_store;
mod mqtt_receiver;
mod websocket;
mod alerts;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::{
    extract::{ws::WebSocketUpgrade, State, Path},
    response::Response,
    routing::get,
    Json, Router,
};
use chrono::Utc;
use parking_lot::Mutex;
use serde::Serialize;
use tower_http::cors::CorsLayer;
use tracing::{info, error, warn, debug};
use uuid::Uuid;

use crate::alerts::{AlertManager, CompensationController};
use crate::clickhouse_store::ClickHouseStore;
use crate::hydraulic::HydraulicModel;
use crate::models::{ClepsydraConfig, HydraulicMetrics, SensorData};
use crate::websocket::{WebSocketBroadcaster, WsMessage};

#[derive(Clone)]
struct AppState {
    broadcaster: Arc<WebSocketBroadcaster>,
    store: Arc<ClickHouseStore>,
    hydraulic_model: Arc<HydraulicModel>,
    alert_manager: Arc<AlertManager>,
    compensation_controller: Arc<CompensationController>,
    daily_error_map: Arc<Mutex<HashMap<String, f64>>>,
    last_update: Arc<Mutex<HashMap<String, chrono::DateTime<Utc>>>>,
    configs: Arc<Mutex<HashMap<String, ClepsydraConfig>>>,
}

#[derive(Serialize)]
struct ApiResponse<T: Serialize> {
    success: bool,
    data: Option<T>,
    message: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    info!("启动古代水运仪象台漏壶水力精度仿真系统...");

    let clickhouse_url = std::env::var("CLICKHOUSE_URL")
        .unwrap_or_else(|_| "http://localhost:8123".to_string());
    let clickhouse_db = std::env::var("CLICKHOUSE_DB")
        .unwrap_or_else(|_| "clepsydra".to_string());
    let mqtt_broker = std::env::var("MQTT_BROKER")
        .unwrap_or_else(|_| "localhost".to_string());
    let mqtt_port: u16 = std::env::var("MQTT_PORT")
        .unwrap_or_else(|_| "1883".to_string())
        .parse()?;
    let mqtt_topic = std::env::var("MQTT_TOPIC")
        .unwrap_or_else(|_| "clepsydra/sensor/+".to_string());
    let server_port: u16 = std::env::var("SERVER_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()?;
    let daily_error_threshold: f64 = std::env::var("DAILY_ERROR_THRESHOLD")
        .unwrap_or_else(|_| "60.0".to_string())
        .parse()?;

    info!("ClickHouse: {}/{}", clickhouse_url, clickhouse_db);
    info!("MQTT: {}:{}, topic: {}", mqtt_broker, mqtt_port, mqtt_topic);
    info!("Server port: {}", server_port);
    info!("日误差阈值: {}秒", daily_error_threshold);

    let store = Arc::new(ClickHouseStore::new(&clickhouse_url, &clickhouse_db)?);
    let broadcaster = WebSocketBroadcaster::new(1000);
    let hydraulic_model = Arc::new(HydraulicModel::new());
    let alert_manager = Arc::new(AlertManager::new(daily_error_threshold));
    let compensation_controller = Arc::new(CompensationController::new());
    let daily_error_map = Arc::new(Mutex::new(HashMap::new()));
    let last_update = Arc::new(Mutex::new(HashMap::new()));

    info!("加载漏壶配置...");
    let configs = Arc::new(Mutex::new(HashMap::new()));
    match store.get_all_configs().await {
        Ok(cfg_list) => {
            let mut map = configs.lock();
            for cfg in cfg_list {
                info!("  {} - {}", cfg.clepsydra_id, cfg.name);
                map.insert(cfg.clepsydra_id.clone(), cfg);
            }
        }
        Err(e) => {
            warn!("加载配置失败（使用默认配置）: {}", e);
            let default_configs = vec![
                ("KD1", "天上壶", 120.0, 20.0, 2.5, 78.54, 0.3, 0.62),
                ("KD2", "夜漏壶", 100.0, 15.0, 2.5, 78.54, 0.3, 0.62),
                ("KD3", "平水壶", 80.0, 10.0, 2.5, 78.54, 0.3, 0.62),
                ("KD4", "万分水", 60.0, 5.0, 2.5, 78.54, 0.3, 0.62),
            ];
            let mut map = configs.lock();
            for (id, name, max_l, min_l, std_flow, area, orifice, coef) in default_configs {
                map.insert(id.to_string(), ClepsydraConfig {
                    clepsydra_id: id.to_string(),
                    name: name.to_string(),
                    max_level: max_l,
                    min_level: min_l,
                    standard_flow: std_flow,
                    cross_section_area: area,
                    orifice_diameter: orifice,
                    flow_coefficient: coef,
                });
            }
        }
    }

    let state = AppState {
        broadcaster: broadcaster.clone(),
        store: store.clone(),
        hydraulic_model: hydraulic_model.clone(),
        alert_manager: alert_manager.clone(),
        compensation_controller: compensation_controller.clone(),
        daily_error_map: daily_error_map.clone(),
        last_update: last_update.clone(),
        configs: configs.clone(),
    };

    let state_clone = state.clone();
    tokio::spawn(async move {
        if let Err(e) = start_mqtt_consumer(
            mqtt_broker,
            mqtt_port,
            &mqtt_topic,
            state_clone,
        ).await {
            error!("MQTT消费者错误: {}", e);
        }
    });

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/configs", get(get_configs))
        .route("/api/sensor/:id", get(get_sensor_data))
        .route("/api/metrics/:id", get(get_metrics))
        .route("/api/alerts/:id", get(get_alerts))
        .route("/api/status", get(get_status))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", server_port)).await?;
    info!("HTTP服务器启动于 http://0.0.0.0:{}", server_port);
    info!("WebSocket端点: ws://0.0.0.0:{}/ws", server_port);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn start_mqtt_consumer(
    broker: String,
    port: u16,
    topic: &str,
    state: AppState,
) -> Result<()> {
    let mut receiver = mqtt_receiver::MqttReceiver::new(
        &broker,
        port,
        &format!("clepsydra-backend-{}", Uuid::new_v4()),
        topic,
    )?;

    receiver.subscribe().await?;

    let state_clone = state.clone();
    receiver.run(move |sensor_data| {
        let state = state_clone.clone();
        tokio::spawn(async move {
            process_sensor_data(state, sensor_data).await;
        });
    }).await?;

    Ok(())
}

async fn process_sensor_data(state: AppState, sensor: SensorData) {
    debug!("收到传感器数据: {} - {:.2}cm", sensor.clepsydra_id, sensor.water_level);

    state.broadcaster.broadcast_sensor_data(&sensor);

    if let Err(e) = state.store.insert_sensor_data(&sensor).await {
        warn!("写入传感器数据失败: {}", e);
    }

    let config = {
        let configs = state.configs.lock();
        configs.get(&sensor.clepsydra_id).cloned()
    };

    if config.is_none() {
        warn!("未知漏壶ID: {}", sensor.clepsydra_id);
        return;
    }
    let config = config.unwrap();

    if let Some(alert) = state.alert_manager.check_water_level(&sensor, &config) {
        state.broadcaster.broadcast_alert(&alert);
        let _ = state.store.insert_alert(&alert).await;
        warn!("水位告警: {}", alert.message);
    }

    if let Some(alert) = state.alert_manager.check_temperature(&sensor) {
        state.broadcaster.broadcast_alert(&alert);
        let _ = state.store.insert_alert(&alert).await;
    }

    let now = Utc::now();
    let dt = {
        let mut last = state.last_update.lock();
        let prev = last.get(&sensor.clepsydra_id).copied().unwrap_or(now);
        let delta = (now - prev).num_milliseconds() as f64 / 1000.0;
        last.insert(sensor.clepsydra_id.clone(), now);
        delta.max(0.1)
    };

    let theoretical_flow = state.hydraulic_model.calculate_theoretical_flow(
        sensor.water_level,
        &config,
        sensor.water_temp,
    );

    let evaporation_rate = state.hydraulic_model.calculate_evaporation_rate(
        sensor.water_temp,
        sensor.humidity,
        config.cross_section_area,
        sensor.quality,
    );

    let flow_error = state.hydraulic_model.calculate_flow_error(
        theoretical_flow,
        sensor.flow_rate,
    );

    let daily_error = {
        let mut errors = state.daily_error_map.lock();
        let current = errors.get(&sensor.clepsydra_id).copied().unwrap_or(0.0);
        let new_error = state.hydraulic_model.update_daily_error(current, flow_error, dt);
        errors.insert(sensor.clepsydra_id.clone(), new_error);
        new_error
    };

    let (compensation_flow, pid_state) = state.compensation_controller.compute_compensation(
        &sensor.clepsydra_id,
        config.standard_flow,
        sensor.flow_rate,
        sensor.water_temp,
        sensor.quality,
        dt,
    );

    let metrics = HydraulicMetrics {
        timestamp: now,
        clepsydra_id: sensor.clepsydra_id.clone(),
        theoretical_flow,
        actual_flow: sensor.flow_rate,
        flow_error,
        evaporation_rate,
        daily_error_seconds: daily_error,
        compensation_flow,
        pid_kp: pid_state.kp,
        pid_ki: pid_state.ki,
        pid_kd: pid_state.kd,
    };

    state.broadcaster.broadcast_metrics(&metrics);

    if let Err(e) = state.store.insert_metrics(&metrics).await {
        warn!("写入水力指标失败: {}", e);
    }

    if let Some(alert) = state.alert_manager.check_daily_error(&metrics) {
        state.broadcaster.broadcast_alert(&alert);
        let _ = state.store.insert_alert(&alert).await;
        warn!("日误差告警: {}", alert.message);
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    let client_id = Uuid::new_v4().to_string();
    let broadcaster = state.broadcaster.clone();

    ws.on_upgrade(|socket| async move {
        use axum::extract::ws::{Message, WebSocket};
        use futures_util::{StreamExt, SinkExt};

        let mut rx = broadcaster.subscribe(client_id.clone());
        let (mut sender, mut receiver) = socket.split();

        let send_task = tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                let json = match serde_json::to_string(&msg) {
                    Ok(s) => s,
                    Err(e) => {
                        error!("序列化WebSocket消息失败: {}", e);
                        continue;
                    }
                };
                if sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        });

        let recv_task = tokio::spawn(async move {
            while let Some(Ok(msg)) = receiver.next().await {
                debug!("收到WebSocket消息: {:?}", msg);
            }
        });

        tokio::select! {
            _ = send_task => {}
            _ = recv_task => {}
        }

        broadcaster.unsubscribe(&client_id);
    })
}

async fn get_configs(State(state): State<AppState>) -> Json<ApiResponse<Vec<ClepsydraConfig>>> {
    let configs = state.configs.lock();
    let config_list: Vec<ClepsydraConfig> = configs.values().cloned().collect();
    Json(ApiResponse {
        success: true,
        data: Some(config_list),
        message: None,
    })
}

async fn get_sensor_data(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<SensorData>>> {
    match state.store.get_recent_sensor_data(&id, 60).await {
        Ok(data) => Json(ApiResponse {
            success: true,
            data: Some(data),
            message: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            message: Some(e.to_string()),
        }),
    }
}

async fn get_metrics(
    Path(_id): Path<String>,
    State(_state): State<AppState>,
) -> Json<ApiResponse<Vec<HydraulicMetrics>>> {
    Json(ApiResponse {
        success: true,
        data: Some(vec![]),
        message: None,
    })
}

async fn get_alerts(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<crate::models::AlertEvent>>> {
    let alerts = state.alert_manager.get_active_alerts(&id);
    Json(ApiResponse {
        success: true,
        data: Some(alerts),
        message: None,
    })
}

async fn get_status(State(state): State<AppState>) -> Json<ApiResponse<serde_json::Value>> {
    let client_count = state.broadcaster.client_count();
    let configs = state.configs.lock();
    let errors = state.daily_error_map.lock();

    let mut clepsydra_status = Vec::new();
    for (id, config) in configs.iter() {
        let daily_error = errors.get(id).copied().unwrap_or(0.0);
        clepsydra_status.push(serde_json::json!({
            "clepsydra_id": id,
            "name": config.name,
            "daily_error_seconds": daily_error,
            "max_level": config.max_level,
            "min_level": config.min_level,
        }));
    }

    Json(ApiResponse {
        success: true,
        data: Some(serde_json::json!({
            "ws_clients": client_count,
            "clepsydras": clepsydra_status,
        })),
        message: None,
    })
}
