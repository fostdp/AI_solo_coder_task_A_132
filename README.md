# 古代水运仪象台漏壶水力精度仿真与误差补偿系统

> 本系统对宋代水运仪象台的四级漏壶系统进行数字化仿真与精度研究，通过 MQTT 采集模拟传感器数据，
> 基于非恒定流与蒸发模型计算计时误差，并通过带前馈控制的 PID 算法进行误差补偿，
> 实现日误差小于 1 分钟的精度目标。

---

## 目录

- [系统架构](#系统架构)
- [技术栈](#技术栈)
- [快速部署](#快速部署)
- [模拟器用法](#模拟器用法)
- [API 文档](#api-文档)
- [Prometheus 指标](#prometheus-指标)
- [ClickHouse 数据分层](#clickhouse-数据分层)
- [本地开发](#本地开发)

---

## 系统架构

```
                             ┌───────────────────────────────┐
                             │        前端 (Nginx)           │
                             │  index.html / js / 3D渲染     │
                             │  Gzip压缩 / 静态资源缓存       │
                             └──────────────┬────────────────┘
                                            │  /ws  /api/
                                            ▼
┌───────────┐   MQTT    ┌───────────────────────────────────────┐
│ 模拟器     │────────▶ │          Rust 后端服务                 │
│ (Python)  │  1883    │  ┌─────────┐  ┌──────────┐  ┌───────┐ │
│ - 四级漏壶 │          │  │dtu_recv │─▶│hydraulic │─▶│error_ │ │
│ - 水温气压 │          │  │  (校验)  │  │ (仿真)    │  │comp   │ │
│ - 异常注入 │          │  └─────────┘  └──────────┘  └───┬───┘ │
└───────────┘          │   tokio::mpsc 管道式异步处理       │     │
                       │                                   ▼     │
                       │   Prometheus /metrics        ┌───────┐ │
                       │   12项指标采集                │alarm_ws│ │
                       │                               └───┬───┘ │
                       └───────────────────────────────────┼─────┘
                                                           │ 写入/推送
                                      ┌────────────────────┼───────────────┐
                                      ▼                    ▼               ▼
                              ┌──────────────┐    ┌───────────────┐  ┌─────────┐
                              │  ClickHouse   │    │  WebSocket    │  │  EMQX   │
                              │  分层存储+TTL │    │  实时推送      │  │  Broker │
                              └──────────────┘    └───────────────┘  └─────────┘
```

**数据流向**：

1. **采集层**：模拟器以 1Hz 频率向 EMQX 发布四级漏壶的传感器数据
2. **处理层**：Rust 后端通过 4 个模块组成的管道式架构处理数据
   - `dtu_receiver`：MQTT 订阅 + 数据校验
   - `hydraulic_simulator`：非恒定流 + 蒸发模型计算
   - `error_compensator`：PID + 前馈控制误差补偿
   - `alarm_ws`：告警检测 + ClickHouse 入库 + WebSocket 广播
3. **存储层**：ClickHouse 四层降采样 + TTL 自动清理
4. **展示层**：前端通过 WebSocket 实时接收数据，Three.js 三维展示

---

## 技术栈

| 层次         | 技术选型                                             |
| ------------ | ---------------------------------------------------- |
| 后端         | Rust 1.75+ / tokio / axum / prometheus / tracing     |
| 数据库       | ClickHouse 24.3 / MergeTree / SummingMergeTree / TTL |
| 消息队列     | EMQX 5.7 (MQTT 3.1.1/5.0)                            |
| 前端         | HTML5 / Three.js / Canvas                            |
| Web 服务器   | Nginx 1.27-alpine / Gzip / 反向代理                   |
| 模拟器       | Python 3.12 / paho-mqtt                              |
| 部署方式     | Docker Compose / 多阶段构建 / 静态二进制             |
| 可观测性     | Prometheus 指标 / tracing 日志                        |

---

## 快速部署

### 环境要求

- Docker ≥ 24.0
- Docker Compose ≥ 2.20
- 至少 4GB 可用内存

### 一键启动

```bash
# 克隆项目后，在项目根目录执行
docker-compose up -d
```

### 服务端口

| 服务        | 端口         | 说明                           |
| ----------- | ------------ | ------------------------------ |
| 前端        | http://localhost:80     | Web 界面（Nginx Gzip压缩）      |
| 后端 API    | http://localhost:8080  | Rust 后端 HTTP 服务             |
| Prometheus  | http://localhost:8080/metrics | 指标端点              |
| EMQX 控制台 | http://localhost:18083 | MQTT Broker 管理台             |
| EMQX MQTT   | 1883         | MQTT 协议端口                   |
| ClickHouse  | 8123 / 9000  | HTTP / Native 接口             |

### 验证部署

```bash
# 查看服务状态
docker-compose ps

# 查看后端日志
docker-compose logs -f backend

# 查看模拟器日志
docker-compose logs -f simulator

# 访问指标端点
curl http://localhost:8080/metrics

# 健康检查
curl http://localhost:8080/api/status
```

### 停止服务

```bash
# 停止并保留数据
docker-compose down

# 停止并清除数据卷（慎用）
docker-compose down -v
```

---

## 模拟器用法

漏壶传感器模拟器位于 `simulator/simulator.py`，可独立运行或通过 Docker 启动。

### 启动方式

**Docker 方式（推荐）**：

```bash
# 使用默认配置启动
docker-compose up -d simulator

# 自定义参数（通过环境变量）
SIM_ALTITUDE=2000 SIM_WATER_TEMP=35 docker-compose up -d simulator
```

**本地运行**：

```bash
cd simulator
pip install -r requirements.txt
python simulator.py --help
```

### 命令行参数

| 参数                  | 环境变量            | 默认值    | 说明                                       |
| --------------------- | ------------------- | --------- | ------------------------------------------ |
| `--host`              | `SIM_MQTT_HOST`     | localhost | MQTT Broker 地址                           |
| `--port`              | `SIM_MQTT_PORT`     | 1883      | MQTT 端口                                  |
| `--interval`          | `SIM_INTERVAL`      | 1.0       | 上报间隔（秒）                             |
| `--altitude`          | `SIM_ALTITUDE`      | 0         | 海拔高度（米），用于计算基准气压           |
| `--water-temp`        | `SIM_WATER_TEMP`    | 20.0      | 基准水温（°C）                             |
| `--pressure`          | `SIM_PRESSURE`      | -         | 固定气压（kPa），覆盖海拔计算              |
| `--temp-variation`    | `SIM_TEMP_VAR`      | 5.0       | 水温日变化幅度（°C）                       |
| `--humidity`          | `SIM_HUMIDITY`      | 60.0      | 环境湿度（%）                              |
| `--quality`           | `SIM_QUALITY`       | 1.0       | 水质系数（0.8-1.2）                        |
| `--abnormal`          | `SIM_ABNORMAL`      | false     | 是否启用水位异常注入                       |
| `--abnormal-interval` | `SIM_ABNORMAL_INT`  | 60        | 异常注入间隔（秒）                         |
| `--abnormal-target`   | `SIM_ABNORMAL_TGT`  | KD1       | 目标漏壶 ID                                |
| `--abnormal-type`     | `SIM_ABNORMAL_TYPE` | low       | 异常类型：low / high / random             |
| `--abnormal-level`    | `SIM_ABNORMAL_LVL`  | 0.3       | 异常程度（0-1）                            |
| `--abnormal-duration` | `SIM_ABNORMAL_DUR`  | 10        | 异常持续时间（秒）                         |

### 使用示例

**模拟高海拔（拉萨 3650m）场景**：

```bash
python simulator.py --altitude 3650 --water-temp 15
```

**模拟高温高湿环境**：

```bash
python simulator.py --water-temp 40 --temp-variation 10 --humidity 80
```

**注入周期性低水位异常**：

```bash
python simulator.py \
  --abnormal \
  --abnormal-interval 120 \
  --abnormal-target KD3 \
  --abnormal-type low \
  --abnormal-level 0.5 \
  --abnormal-duration 20
```

**随机异常测试**：

```bash
python simulator.py --abnormal --abnormal-type random --abnormal-level 0.8
```

### MQTT 主题

- 上报主题：`clepsydra/sensor/{clepsydra_id}`
- 漏壶编号：`KD1`（天上壶）、`KD2`（夜漏壶）、`KD3`（平水壶）、`KD4`（万分水）

消息格式示例：

```json
{
  "clepsydra_id": "KD1",
  "water_level": 85.5,
  "flow_rate": 2.48,
  "water_temp": 22.3,
  "humidity": 65.0,
  "quality": 1.02,
  "pressure": 101.325
}
```

---

## API 文档

### HTTP 接口

| 方法 | 路径           | 说明                       |
| ---- | -------------- | -------------------------- |
| GET  | `/api/status`   | 服务健康检查               |
| GET  | `/metrics`      | Prometheus 指标（文本格式）|
| GET  | `/api/alerts`   | 最近告警列表（待实现）     |
| WS   | `/ws`           | WebSocket 实时数据推送     |

### WebSocket 消息

连接成功后，服务端会持续推送以下类型的消息：

**传感器数据更新**：

```json
{
  "type": "sensor_update",
  "clepsydra_id": "KD1",
  "water_level": 85.5,
  "flow_rate": 2.48,
  "water_temp": 22.3,
  "pressure": 101.325,
  "timestamp": "2024-01-15T10:30:00.000+08:00"
}
```

**水力指标更新**：

```json
{
  "type": "metrics_update",
  "clepsydra_id": "KD1",
  "theoretical_flow": 2.5,
  "flow_error": 0.8,
  "evaporation_rate": 0.002,
  "daily_error_seconds": 12.5,
  "compensation_flow": 0.05
}
```

**告警事件**：

```json
{
  "type": "alert",
  "id": "uuid",
  "clepsydra_id": "KD3",
  "alert_type": "WATER_LEVEL_LOW",
  "alert_level": "WARNING",
  "message": "平水壶水位低于下限",
  "value": 8.5,
  "threshold": 10.0,
  "timestamp": "2024-01-15T10:30:00.000+08:00"
}
```

---

## Prometheus 指标

访问 `http://localhost:8080/metrics` 获取全部指标。

### 指标列表

| 指标名称                           | 类型      | 标签             | 说明                       |
| ---------------------------------- | --------- | ---------------- | -------------------------- |
| `clepsydra_sensor_received_total`  | Counter   | clepsydra_id     | 传感器消息接收总数         |
| `clepsydra_validation_errors_total`| Counter   | error_type       | 数据校验错误总数           |
| `clepsydra_water_level_cm`         | Gauge     | clepsydra_id     | 当前水位（cm）             |
| `clepsydra_flow_rate_mlps`         | Gauge     | clepsydra_id     | 实际流量（mL/s）           |
| `clepsydra_theoretical_flow_mlps`  | Gauge     | clepsydra_id     | 理论流量（mL/s）           |
| `clepsydra_flow_error_percent`     | Gauge     | clepsydra_id     | 流量误差率（%）            |
| `clepsydra_evaporation_rate_mlps`  | Gauge     | clepsydra_id     | 蒸发速率（mL/s）           |
| `clepsydra_daily_error_seconds`    | Gauge     | clepsydra_id     | 日累计误差（秒）           |
| `clepsydra_compensation_flow_mlps` | Gauge     | clepsydra_id     | PID补偿流量（mL/s）        |
| `clepsydra_alerts_total`           | Counter   | type, level      | 告警触发总数               |
| `clepsydra_ws_clients`             | Gauge     | -                | WebSocket 连接数           |
| `clepsydra_processing_duration_s`  | Histogram | stage            | 各阶段处理耗时（秒）       |

### Grafana 集成

在 Prometheus 中添加抓取配置：

```yaml
scrape_configs:
  - job_name: 'clepsydra-backend'
    static_configs:
      - targets: ['backend:8080']
    metrics_path: '/metrics'
    scrape_interval: 5s
```

---

## ClickHouse 数据分层

系统采用四层降采样 + TTL 自动生命周期管理，在查询性能与存储成本间取得平衡。

| 层级     | 粒度  | 保留期 | 存储引擎         | 表名                      |
| -------- | ----- | ------ | ---------------- | ------------------------- |
| 原始层   | 秒级  | 7 天   | MergeTree        | `sensor_data`             |
| 明细层   | 分钟  | 30 天  | SummingMergeTree | `sensor_data_1min`        |
| 聚合层   | 小时  | 1 年   | SummingMergeTree | `sensor_data_1hour`       |
| 归档层   | 天    | 3 年   | SummingMergeTree | `daily_error_summary`     |

**物化视图自动链路**：

```
sensor_data (秒级)
    └─► sensor_data_1min_mv ──► sensor_data_1min (分钟)
                                    └─► sensor_data_1hour_mv ──► sensor_data_1hour (小时)
                                                                    └─► daily_error_summary_mv ──► daily_error_summary (日)
```

### 常用查询

```sql
-- 最近 1 小时水位趋势（小时粒度，秒级也可查但仅限7天内）
SELECT timestamp, avg_water_level
FROM clepsydra.sensor_data_1hour
WHERE clepsydra_id = 'KD3'
  AND timestamp >= now() - INTERVAL 1 HOUR
ORDER BY timestamp;

-- 过去 30 天日误差趋势
SELECT date, max_daily_error, avg_daily_error
FROM clepsydra.daily_error_summary
WHERE clepsydra_id = 'KD1'
ORDER BY date;
```

---

## 本地开发

### 后端开发

```bash
cd backend
cargo check
cargo run
```

### 前端开发

直接用浏览器打开 `frontend/index.html`，或使用任意静态文件服务器：

```bash
cd frontend
python -m http.server 3000
# 访问 http://localhost:3000
```

### 目录结构

```
.
├── backend/                 # Rust 后端
│   ├── src/
│   │   ├── main.rs          # 主入口
│   │   ├── metrics.rs       # Prometheus 指标
│   │   ├── dtu_receiver.rs  # DTU接收模块
│   │   ├── hydraulic_simulator.rs  # 水力仿真
│   │   ├── error_compensator.rs    # 误差补偿
│   │   ├── alarm_ws.rs      # 告警与WS推送
│   │   ├── config_loader.rs # 配置加载
│   │   └── ...
│   ├── config/app_config.json
│   ├── Cargo.toml
│   └── Dockerfile           # 多阶段构建
├── clickhouse/
│   └── init.sql             # 初始化脚本（降采样+TTL）
├── frontend/
│   ├── index.html
│   ├── js/                  # 前端模块
│   ├── src/                 # Three.js组件
│   ├── nginx.conf           # Nginx配置（含Gzip）
│   └── Dockerfile
├── simulator/
│   ├── simulator.py         # 传感器模拟器
│   └── Dockerfile
└── docker-compose.yml
```

---

## License

本项目用于科技史研究与教学目的。
