mod models;
mod hydraulic;
mod clickhouse_store;
mod mqtt_receiver;
mod websocket;
mod alerts;
mod config_loader;
mod dtu_receiver;
mod hydraulic_simulator;
mod error_compensator;
mod alarm_ws;
mod metrics;

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
use parking_lot::Mutex;
use serde::Serialize;
use tower_http::cors::CorsLayer;
use tracing::{info, error, warn};
use uuid::Uuid;

use crate::alerts::AlertManager;
use crate::alarm_ws::AlarmWsService;
use crate::clickhouse_store::ClickHouseStore;
use crate::config_loader::AppConfig;
use crate::dtu_receiver::{DtuReceiver, ValidatedSensor};
use crate::error_compensator::{CompensatedOutput, ErrorCompensator};
use crate::hydraulic::HydraulicModel;
use crate::hydraulic_simulator::{HydraulicSimulator, SimulatorOutput};
use crate::models::{ClepsydraConfig, HydraulicMetrics, PidState, SensorData};
use crate::websocket::WebSocketBroadcaster;
use crate::metrics::{gather_metrics_text, set_ws_clients, init_metrics};

#[derive(Clone)]
struct AppState {
    config: Arc<AppConfig>,
    broadcaster: Arc<WebSocketBroadcaster>,
    store: Arc<ClickHouseStore>,
    alert_manager: Arc<AlertManager>,
    daily_error_map: Arc<Mutex<HashMap<String, f64>>>,
    last_update: Arc<Mutex<HashMap<String, chrono::DateTime<chrono::Utc>>>>,
    pid_states: Arc<Mutex<HashMap<String, PidState>>>,
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

    let _metrics_registry = init_metrics();

    let config_path = std::env::var("APP_CONFIG")
        .unwrap_or_else(|_| "config/app_config.json".to_string());
    let mut config = AppConfig::load_from_file(&config_path)
        .unwrap_or_else(|e| {
            warn!("加载配置文件失败，使用内置默认: {}", e);
            AppConfig::load_from_file("config/app_config.json")
                .expect("内置默认配置必须存在")
        });

    let clickhouse_url = std::env::var("CLICKHOUSE_URL")
        .unwrap_or_else(|_| config.clickhouse.url.clone());
    let clickhouse_db = std::env::var("CLICKHOUSE_DB")
        .unwrap_or_else(|_| config.clickhouse.database.clone());
    let server_port: u16 = std::env::var("SERVER_PORT")
        .unwrap_or_else(|_| config.server.port.to_string())
        .parse()?;
    if let Ok(v) = std::env::var("MQTT_BROKER") { config.mqtt.broker = v; }
    if let Ok(v) = std::env::var("MQTT_PORT") {
        config.mqtt.port = v.parse().unwrap_or(config.mqtt.port);
    }

    info!("ClickHouse: {}/{}", clickhouse_url, clickhouse_db);
    info!("MQTT: {}:{}, topic: {}", config.mqtt.broker, config.mqtt.port, config.mqtt.topic);
    info!("Server port: {}", server_port);
    info!("日误差阈值: {}秒", config.alerts.daily_error_threshold_seconds);

    let store = Arc::new(ClickHouseStore::new(&clickhouse_url, &clickhouse_db)?);
    let broadcaster = WebSocketBroadcaster::new(
        config.channels.alarm_broadcast_capacity,
    );
    let hydraulic_model = Arc::new(HydraulicModel::new());
    let alert_manager = Arc::new(AlertManager::new(
        config.alerts.daily_error_threshold_seconds,
    ));

    let configs: Arc<Mutex<HashMap<String, ClepsydraConfig>>> = Arc::new(Mutex::new(HashMap::new()));
    match store.get_all_configs().await {
        Ok(cfg_list) => {
            let mut map = configs.lock();
            for cfg in cfg_list {
                info!("  {} - {} (DB)", cfg.clepsydra_id, cfg.name);
                map.insert(cfg.clepsydra_id.clone(), cfg);
            }
        }
        Err(e) => {
            warn!("加载DB配置失败，使用JSON文件配置: {}", e);
            let mut map = configs.lock();
            for (id, cfg) in config.to_clepsydra_map() {
                info!("  {} - {} (JSON)", id, cfg.name);
                map.insert(id, cfg);
            }
        }
    }

    let cfg_arc = Arc::new(config);

    let (dtu_tx, sim_rx) = tokio::sync::mpsc::channel::<ValidatedSensor>(
        cfg_arc.channels.dtu_to_simulator_buffer,
    );
    let (sim_tx, comp_rx) = tokio::sync::mpsc::channel::<SimulatorOutput>(
        cfg_arc.channels.simulator_to_compensator_buffer,
    );
    let (comp_tx, alm_rx) = tokio::sync::mpsc::channel::<CompensatedOutput>(
        cfg_arc.channels.compensator_to_alarm_buffer,
    );

    let simulator = HydraulicSimulator::new(
        cfg_arc.clone(),
        configs.clone(),
        hydraulic_model.clone(),
        sim_rx,
        sim_tx,
    );
    let (last_update_ref, daily_error_ref) = simulator.get_state_refs();

    let compensator = ErrorCompensator::new(cfg_arc.clone(), comp_rx, comp_tx);
    let pid_states_ref = compensator.get_pid_states_ref();

    let alarm_ws = AlarmWsService::new(
        alert_manager.clone(),
        store.clone(),
        broadcaster.clone(),
        alm_rx,
    );

    let state = AppState {
        config: cfg_arc.clone(),
        broadcaster: broadcaster.clone(),
        store: store.clone(),
        alert_manager: alert_manager.clone(),
        daily_error_map: daily_error_ref.clone(),
        last_update: last_update_ref.clone(),
        pid_states: pid_states_ref.clone(),
        configs: configs.clone(),
    };

    let dtu = DtuReceiver::new(cfg_arc.clone(), dtu_tx);
    tokio::spawn(async move {
        if let Err(e) = dtu.run().await {
            error!("[DTU] 任务异常退出: {}", e);
        }
    });
    tokio::spawn(async move {
        if let Err(e) = simulator.run().await {
            error!("[SIM] 任务异常退出: {}", e);
        }
    });
    tokio::spawn(async move {
        if let Err(e) = compensator.run().await {
            error!("[PID] 任务异常退出: {}", e);
        }
    });
    tokio::spawn(async move {
        if let Err(e) = alarm_ws.run().await {
            error!("[ALM] 任务异常退出: {}", e);
        }
    });

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/metrics", get(metrics_handler))
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

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    let client_id = Uuid::new_v4().to_string();
    let broadcaster = state.broadcaster.clone();

    ws.on_upgrade(move |socket| async move {
        use axum::extract::ws::{Message, WebSocket};
        use futures_util::{StreamExt, SinkExt};

        set_ws_clients(broadcaster.client_count() as f64 + 1.0);
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

        let broadcaster_clone = broadcaster.clone();
        let client_id_clone = client_id.clone();
        let recv_task = tokio::spawn(async move {
            while let Some(Ok(msg)) = receiver.next().await {
                tracing::debug!("收到WebSocket消息: {:?}", msg);
            }
            drop(broadcaster_clone);
            drop(client_id_clone);
        });

        tokio::select! {
            _ = send_task => {}
            _ = recv_task => {}
        }

        broadcaster.unsubscribe(&client_id);
        set_ws_clients(broadcaster.client_count() as f64 - 1.0);
    })
}

async fn get_configs(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<ClepsydraConfig>>> {
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

async fn get_status(
    State(state): State<AppState>,
) -> Json<ApiResponse<serde_json::Value>> {
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
            "mqtt_broker": state.config.mqtt.broker,
            "clickhouse_url": state.config.clickhouse.url,
        })),
        message: None,
    })
}

async fn metrics_handler() -> String {
    gather_metrics_text()
}
