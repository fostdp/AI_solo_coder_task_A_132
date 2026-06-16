-- 古代水运仪象台漏壶水力精度仿真系统 - ClickHouse 初始化脚本

CREATE DATABASE IF NOT EXISTS clepsydra
ENGINE = Atomic;

USE clepsydra;

-- 漏壶传感器原始数据表
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
PARTITION BY toYYYYMM(timestamp)
ORDER BY (clepsydra_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR
COMMENT '漏壶传感器秒级原始数据';

-- 水力精度计算结果表
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
PARTITION BY toYYYYMM(timestamp)
ORDER BY (clepsydra_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR
COMMENT '水力精度仿真与PID补偿计算结果';

-- 告警事件表
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
TTL timestamp + INTERVAL 6 MONTH
COMMENT '告警事件记录表';

-- 漏壶配置参数表
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

-- 日误差统计物化视图
CREATE MATERIALIZED VIEW IF NOT EXISTS daily_error_summary_mv
TO daily_error_summary
AS
SELECT
    toDate(timestamp) AS date,
    clepsydra_id,
    max(daily_error_seconds) AS max_daily_error,
    avg(daily_error_seconds) AS avg_daily_error,
    min(daily_error_seconds) AS min_daily_error,
    count() AS data_points
FROM hydraulic_metrics
GROUP BY date, clepsydra_id;

CREATE TABLE IF NOT EXISTS daily_error_summary (
    date Date,
    clepsydra_id String,
    max_daily_error Float64,
    avg_daily_error Float64,
    min_daily_error Float64,
    data_points UInt64
)
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(date)
ORDER BY (date, clepsydra_id)
COMMENT '日误差统计汇总';

-- 水位异常检测
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
COMMENT '水位异常检测结果';
