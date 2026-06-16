use std::collections::HashSet;
use std::sync::Arc;
use parking_lot::Mutex;
use tokio::sync::broadcast;
use serde::Serialize;
use tracing::{info, debug, warn};
use crate::models::{SensorData, HydraulicMetrics, AlertEvent};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    SensorData(SensorData),
    HydraulicMetrics(HydraulicMetrics),
    Alert(AlertEvent),
    Status(StatusMessage),
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusMessage {
    pub status: String,
    pub message: String,
}

pub struct WebSocketBroadcaster {
    sender: broadcast::Sender<WsMessage>,
    connected_clients: Mutex<HashSet<String>>,
}

impl WebSocketBroadcaster {
    pub fn new(capacity: usize) -> Arc<Self> {
        let (sender, _) = broadcast::channel(capacity);
        Arc::new(Self {
            sender,
            connected_clients: Mutex::new(HashSet::new()),
        })
    }

    pub fn subscribe(&self, client_id: String) -> broadcast::Receiver<WsMessage> {
        let mut clients = self.connected_clients.lock();
        clients.insert(client_id.clone());
        info!("WebSocket client connected: {}, total: {}", client_id, clients.len());
        self.sender.subscribe()
    }

    pub fn unsubscribe(&self, client_id: &str) {
        let mut clients = self.connected_clients.lock();
        clients.remove(client_id);
        info!("WebSocket client disconnected: {}, total: {}", client_id, clients.len());
    }

    pub fn broadcast_sensor_data(&self, data: &SensorData) {
        let msg = WsMessage::SensorData(data.clone());
        if let Err(e) = self.sender.send(msg) {
            debug!("No WebSocket receivers for sensor data: {}", e);
        }
    }

    pub fn broadcast_metrics(&self, metrics: &HydraulicMetrics) {
        let msg = WsMessage::HydraulicMetrics(metrics.clone());
        if let Err(e) = self.sender.send(msg) {
            debug!("No WebSocket receivers for metrics: {}", e);
        }
    }

    pub fn broadcast_alert(&self, alert: &AlertEvent) {
        let msg = WsMessage::Alert(alert.clone());
        match self.sender.send(msg) {
            Ok(n) => debug!("Alert broadcast to {} receivers", n),
            Err(e) => warn!("Failed to broadcast alert: {}", e),
        }
    }

    pub fn broadcast_status(&self, status: &str, message: &str) {
        let msg = WsMessage::Status(StatusMessage {
            status: status.to_string(),
            message: message.to_string(),
        });
        let _ = self.sender.send(msg);
    }

    pub fn client_count(&self) -> usize {
        self.connected_clients.lock().len()
    }
}
