use std::sync::Arc;
use anyhow::Result;
use tracing::{debug, warn, error};

use crate::error_compensator::CompensatedOutput;
use crate::alerts::AlertManager;
use crate::clickhouse_store::ClickHouseStore;
use crate::websocket::WebSocketBroadcaster;
use crate::metrics::inc_alert;

pub struct AlarmWsService {
    alert_manager: Arc<AlertManager>,
    store: Arc<ClickHouseStore>,
    broadcaster: Arc<WebSocketBroadcaster>,
    rx: tokio::sync::mpsc::Receiver<CompensatedOutput>,
}

impl AlarmWsService {
    pub fn new(
        alert_manager: Arc<AlertManager>,
        store: Arc<ClickHouseStore>,
        broadcaster: Arc<WebSocketBroadcaster>,
        rx: tokio::sync::mpsc::Receiver<CompensatedOutput>,
    ) -> Self {
        Self { alert_manager, store, broadcaster, rx }
    }

    pub fn get_refs(
        &self,
    ) -> (
        Arc<AlertManager>,
        Arc<ClickHouseStore>,
        Arc<WebSocketBroadcaster>,
    ) {
        (
            self.alert_manager.clone(),
            self.store.clone(),
            self.broadcaster.clone(),
        )
    }

    async fn persist_and_push(&self, alert: &crate::models::AlertEvent) {
        inc_alert(
            &alert.clepsydra_id,
            &alert.alert_type.as_str(),
            &alert.alert_level.as_str(),
        );
        self.broadcaster.broadcast_alert(alert);
        if let Err(e) = self.store.insert_alert(alert).await {
            warn!("[ALM] 告警入库失败: {}", e);
        }
    }

    pub async fn run(mut self) -> Result<()> {
        debug!("[ALM] 告警与推送服务启动");

        while let Some(output) = self.rx.recv().await {
            let CompensatedOutput { sensor, metrics, config } = output;

            self.broadcaster.broadcast_sensor_data(&sensor);
            self.broadcaster.broadcast_metrics(&metrics);

            if let Err(e) = self.store.insert_sensor_data(&sensor).await {
                warn!("[ALM] 传感器数据入库失败: {}", e);
            }
            if let Err(e) = self.store.insert_metrics(&metrics).await {
                warn!("[ALM] 水力指标入库失败: {}", e);
            }

            if let Some(alert) = self.alert_manager.check_water_level(&sensor, &config) {
                warn!("[ALM] 水位告警: {}", alert.message);
                self.persist_and_push(&alert).await;
            }

            if let Some(alert) = self.alert_manager.check_temperature(&sensor) {
                warn!("[ALM] 温度告警: {}", alert.message);
                self.persist_and_push(&alert).await;
            }

            if let Some(alert) = self.alert_manager.check_daily_error(&metrics) {
                warn!("[ALM] 日误差告警: {}", alert.message);
                self.persist_and_push(&alert).await;
            }
        }

        debug!("[ALM] 告警与推送服务退出");
        Ok(())
    }
}
