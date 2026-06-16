-- ============================================================
-- 古代水运仪象台漏壶水力精度仿真系统 - ClickHouse 初始化脚本
-- 分层存储：秒级原始(7天) → 分钟聚合(30天) → 小时聚合(1年) → 日聚合(3年)
-- ============================================================

CREATE DATABASE IF NOT EXISTS clepsydra
ENGINE = Atomic;

USE clepsydra;

-- ============================================================
-- 1. 原始数据表（秒级，保留 7 天）
-- ============================================================

CREATE TABLE IF NOT EXISTS sensor_data (
    timestamp DateTime64(3, 'Asia/Shanghai') DEFAULT now64(3),
    clepsydra_id String COMMENT '漏壶编号: KD1-天上壶, KD2-夜漏壶, KD3-平水壶, KD4-万分水',
    water_level Float64 COMMENT '水位高度 (cm)',
    flow_rate Float64 COMMENT '流量 (mL/s)',
    water_temp Float64 COMMENT '水温 (°C)',
    humidity Float64 COMMENT '环境湿度 (%)',
    quality Float64 COMMENT '水质系数 (0.8-1.2)',
    pressure Float64 DEFAULT 101.325 COMMENT '大气压 (kPa)',
    received_at DateTime64(3, 'Asia/Shanghai') DEFAULT now64(3)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMMDD(timestamp)
ORDER BY (clepsydra_id, timestamp)
TTL toDateTime(timestamp) + INTERVAL 7 DAY
COMMENT '漏壶传感器秒级原始数据（保留7天）';

CREATE TABLE IF NOT EXISTS hydraulic_metrics (
    timestamp DateTime64(3, 'Asia/Shanghai') DEFAULT now64(3),
    clepsydra_id String,
    theoretical_flow Float64 COMMENT '理论流量 (mL/s)',
    actual_flow Float64 COMMENT '实际流量 (mL/s)',
    flow_error Float64 COMMENT '流量误差率 (%)',
    evaporation_rate Float64 COMMENT '蒸发速率 (mL/s)',
    daily_error_seconds Float64 COMMENT '日累计计时误差 (秒)',
    compensation_flow Float64 COMMENT 'PID补偿流量 (mL/s)',
    pid_kp Float64 COMMENT 'PID比例系数',
    pid_ki Float64 COMMENT 'PID积分系数',
    pid_kd Float64 COMMENT 'PID微分系数'
)
ENGINE = MergeTree()
PARTITION BY toYYYYMMDD(timestamp)
ORDER BY (clepsydra_id, timestamp)
TTL toDateTime(timestamp) + INTERVAL 7 DAY
COMMENT '水力精度仿真与PID补偿计算结果（保留7天）';

-- ============================================================
-- 2. 分钟级聚合表（保留 30 天）
-- ============================================================

CREATE TABLE IF NOT EXISTS sensor_data_1min (
    timestamp DateTime('Asia/Shanghai'),
    clepsydra_id String,
    avg_water_level Float64,
    min_water_level Float64,
    max_water_level Float64,
    avg_flow_rate Float64,
    min_flow_rate Float64,
    max_flow_rate Float64,
    avg_water_temp Float64,
    avg_humidity Float64,
    avg_quality Float64,
    avg_pressure Float64,
    sample_count UInt64
)
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (clepsydra_id, timestamp)
TTL timestamp + INTERVAL 30 DAY
COMMENT '传感器数据分钟级聚合（保留30天）';

CREATE MATERIALIZED VIEW IF NOT EXISTS sensor_data_1min_mv
TO sensor_data_1min
AS
SELECT
    toStartOfMinute(timestamp) AS timestamp,
    clepsydra_id,
    avg(water_level) AS avg_water_level,
    min(water_level) AS min_water_level,
    max(water_level) AS max_water_level,
    avg(flow_rate) AS avg_flow_rate,
    min(flow_rate) AS min_flow_rate,
    max(flow_rate) AS max_flow_rate,
    avg(water_temp) AS avg_water_temp,
    avg(humidity) AS avg_humidity,
    avg(quality) AS avg_quality,
    avg(pressure) AS avg_pressure,
    count() AS sample_count
FROM sensor_data
GROUP BY timestamp, clepsydra_id;

CREATE TABLE IF NOT EXISTS hydraulic_metrics_1min (
    timestamp DateTime('Asia/Shanghai'),
    clepsydra_id String,
    avg_theoretical_flow Float64,
    avg_actual_flow Float64,
    avg_flow_error Float64,
    max_flow_error Float64,
    avg_evaporation_rate Float64,
    avg_daily_error Float64,
    max_daily_error Float64,
    avg_compensation_flow Float64,
    sample_count UInt64
)
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (clepsydra_id, timestamp)
TTL timestamp + INTERVAL 30 DAY
COMMENT '水力指标分钟级聚合（保留30天）';

CREATE MATERIALIZED VIEW IF NOT EXISTS hydraulic_metrics_1min_mv
TO hydraulic_metrics_1min
AS
SELECT
    toStartOfMinute(timestamp) AS timestamp,
    clepsydra_id,
    avg(theoretical_flow) AS avg_theoretical_flow,
    avg(actual_flow) AS avg_actual_flow,
    avg(flow_error) AS avg_flow_error,
    max(flow_error) AS max_flow_error,
    avg(evaporation_rate) AS avg_evaporation_rate,
    avg(daily_error_seconds) AS avg_daily_error,
    max(daily_error_seconds) AS max_daily_error,
    avg(compensation_flow) AS avg_compensation_flow,
    count() AS sample_count
FROM hydraulic_metrics
GROUP BY timestamp, clepsydra_id;

-- ============================================================
-- 3. 小时级聚合表（保留 1 年）
-- ============================================================

CREATE TABLE IF NOT EXISTS sensor_data_1hour (
    timestamp DateTime('Asia/Shanghai'),
    clepsydra_id String,
    avg_water_level Float64,
    min_water_level Float64,
    max_water_level Float64,
    avg_flow_rate Float64,
    min_flow_rate Float64,
    max_flow_rate Float64,
    avg_water_temp Float64,
    min_water_temp Float64,
    max_water_temp Float64,
    avg_humidity Float64,
    avg_quality Float64,
    avg_pressure Float64,
    sample_count UInt64
)
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (clepsydra_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR
COMMENT '传感器数据小时级聚合（保留1年）';

CREATE MATERIALIZED VIEW IF NOT EXISTS sensor_data_1hour_mv
TO sensor_data_1hour
AS
SELECT
    toStartOfHour(timestamp) AS timestamp,
    clepsydra_id,
    avg(avg_water_level) AS avg_water_level,
    min(min_water_level) AS min_water_level,
    max(max_water_level) AS max_water_level,
    avg(avg_flow_rate) AS avg_flow_rate,
    min(min_flow_rate) AS min_flow_rate,
    max(max_flow_rate) AS max_flow_rate,
    avg(avg_water_temp) AS avg_water_temp,
    min(avg_water_temp) AS min_water_temp,
    max(avg_water_temp) AS max_water_temp,
    avg(avg_humidity) AS avg_humidity,
    avg(avg_quality) AS avg_quality,
    avg(avg_pressure) AS avg_pressure,
    sum(sample_count) AS sample_count
FROM sensor_data_1min
GROUP BY timestamp, clepsydra_id;

CREATE TABLE IF NOT EXISTS hydraulic_metrics_1hour (
    timestamp DateTime('Asia/Shanghai'),
    clepsydra_id String,
    avg_theoretical_flow Float64,
    avg_actual_flow Float64,
    avg_flow_error Float64,
    max_flow_error Float64,
    avg_evaporation_rate Float64,
    avg_daily_error Float64,
    max_daily_error Float64,
    avg_compensation_flow Float64,
    sample_count UInt64
)
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (clepsydra_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR
COMMENT '水力指标小时级聚合（保留1年）';

CREATE MATERIALIZED VIEW IF NOT EXISTS hydraulic_metrics_1hour_mv
TO hydraulic_metrics_1hour
AS
SELECT
    toStartOfHour(timestamp) AS timestamp,
    clepsydra_id,
    avg(avg_theoretical_flow) AS avg_theoretical_flow,
    avg(avg_actual_flow) AS avg_actual_flow,
    avg(avg_flow_error) AS avg_flow_error,
    max(max_flow_error) AS max_flow_error,
    avg(avg_evaporation_rate) AS avg_evaporation_rate,
    avg(avg_daily_error) AS avg_daily_error,
    max(max_daily_error) AS max_daily_error,
    avg(avg_compensation_flow) AS avg_compensation_flow,
    sum(sample_count) AS sample_count
FROM hydraulic_metrics_1min
GROUP BY timestamp, clepsydra_id;

-- ============================================================
-- 4. 日级聚合表（保留 3 年）
-- ============================================================

CREATE TABLE IF NOT EXISTS daily_error_summary (
    date Date,
    clepsydra_id String,
    max_daily_error Float64,
    avg_daily_error Float64,
    min_daily_error Float64,
    avg_theoretical_flow Float64,
    avg_actual_flow Float64,
    avg_evaporation_rate Float64,
    avg_compensation_flow Float64,
    max_water_level Float64,
    min_water_level Float64,
    avg_water_temp Float64,
    avg_pressure Float64,
    data_points UInt64
)
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(date)
ORDER BY (date, clepsydra_id)
TTL date + INTERVAL 3 YEAR
COMMENT '日级误差与精度统计汇总（保留3年）';

CREATE MATERIALIZED VIEW IF NOT EXISTS daily_error_summary_mv
TO daily_error_summary
AS
SELECT
    toDate(timestamp) AS date,
    clepsydra_id,
    max(max_daily_error) AS max_daily_error,
    avg(avg_daily_error) AS avg_daily_error,
    min(avg_daily_error) AS min_daily_error,
    avg(avg_theoretical_flow) AS avg_theoretical_flow,
    avg(avg_actual_flow) AS avg_actual_flow,
    avg(avg_evaporation_rate) AS avg_evaporation_rate,
    avg(avg_compensation_flow) AS avg_compensation_flow,
    max(max_water_level) AS max_water_level,
    min(min_water_level) AS min_water_level,
    avg(avg_water_temp) AS avg_water_temp,
    avg(avg_pressure) AS avg_pressure,
    sum(sample_count) AS data_points
FROM hydraulic_metrics_1hour
    INNER JOIN sensor_data_1hour USING (timestamp, clepsydra_id)
GROUP BY date, clepsydra_id;

-- ============================================================
-- 5. 告警事件表（保留 1 年）
-- ============================================================

CREATE TABLE IF NOT EXISTS alerts (
    id UUID DEFAULT generateUUIDv4(),
    timestamp DateTime64(3, 'Asia/Shanghai') DEFAULT now64(3),
    clepsydra_id String,
    alert_type String COMMENT '告警类型: WATER_LEVEL_HIGH, WATER_LEVEL_LOW, DAILY_ERROR_EXCEED, TEMP_ABNORMAL',
    alert_level String COMMENT '告警级别: INFO, WARNING, CRITICAL',
    message String,
    value Float64 COMMENT '触发告警的数值',
    threshold Float64 COMMENT '告警阈值',
    resolved UInt8 DEFAULT 0 COMMENT '是否已解决',
    resolved_at DateTime64(3, 'Asia/Shanghai')
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (alert_type, timestamp)
TTL toDateTime(timestamp) + INTERVAL 1 YEAR
COMMENT '告警事件记录表（保留1年）';

-- ============================================================
-- 6. 漏壶配置参数表
-- ============================================================

CREATE TABLE IF NOT EXISTS clepsydra_config (
    clepsydra_id String,
    name String COMMENT '漏壶名称',
    max_level Float64 COMMENT '最高水位 (cm)',
    min_level Float64 COMMENT '最低水位 (cm)',
    standard_flow Float64 COMMENT '标准流量 (mL/s)',
    cross_section_area Float64 COMMENT '横截面积 (cm²)',
    orifice_diameter Float64 COMMENT '出水孔直径 (cm)',
    flow_coefficient Float64 COMMENT '流量系数',
    updated_at DateTime64(3, 'Asia/Shanghai') DEFAULT now64(3)
)
ENGINE = ReplacingMergeTree(updated_at)
ORDER BY clepsydra_id
COMMENT '漏壶配置参数';

-- 插入初始漏壶配置（宋代水运仪象台四级漏壶）
INSERT INTO clepsydra_config (clepsydra_id, name, max_level, min_level, standard_flow, cross_section_area, orifice_diameter, flow_coefficient) VALUES
('KD1', '天上壶', 120.0, 20.0, 2.5, 78.54, 0.3, 0.62),
('KD2', '夜漏壶', 100.0, 15.0, 2.5, 78.54, 0.3, 0.62),
('KD3', '平水壶', 80.0, 10.0, 2.5, 78.54, 0.3, 0.62),
('KD4', '万分水', 60.0, 5.0, 2.5, 78.54, 0.3, 0.62);

-- ============================================================
-- 7. 水位异常检测结果表（保留 30 天）
-- ============================================================

CREATE TABLE IF NOT EXISTS water_level_alerts (
    timestamp DateTime64(3, 'Asia/Shanghai'),
    clepsydra_id String,
    water_level Float64,
    max_level Float64,
    min_level Float64,
    is_high UInt8,
    is_low UInt8
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (clepsydra_id, timestamp)
TTL toDateTime(timestamp) + INTERVAL 30 DAY
COMMENT '水位异常检测结果（保留30天）';

-- ============================================================
-- 8. 系统元数据表
-- ============================================================

CREATE TABLE IF NOT EXISTS system_info (
    key String,
    value String,
    updated_at DateTime64(3, 'Asia/Shanghai') DEFAULT now64(3)
)
ENGINE = ReplacingMergeTree(updated_at)
ORDER BY key
COMMENT '系统元信息';

INSERT INTO system_info (key, value) VALUES
('schema_version', '2.0'),
('initialized_at', toString(now64(3))),
('description', '古代水运仪象台漏壶水力精度仿真系统数据库');
