#!/usr/bin/env python3
"""
古代水运仪象台漏壶传感器模拟器（增强版）
模拟四级漏壶（天上壶、夜漏壶、平水壶、万分水）的传感器数据
支持自定义水温和气压条件、水位异常注入
"""

import json
import time
import random
import math
import argparse
import os
import sys
from datetime import datetime

try:
    import paho.mqtt.client as mqtt
except ImportError:
    print("请先安装依赖: pip install paho-mqtt")
    exit(1)


CLEPSYDRAS = [
    {
        "id": "KD1",
        "name": "天上壶",
        "max_level": 120.0,
        "min_level": 20.0,
        "init_level": 100.0,
        "base_flow": 2.5,
        "cross_section": 78.54,
        "orifice_diameter": 0.3,
        "flow_coefficient": 0.62,
    },
    {
        "id": "KD2",
        "name": "夜漏壶",
        "max_level": 100.0,
        "min_level": 15.0,
        "init_level": 85.0,
        "base_flow": 2.5,
        "cross_section": 78.54,
        "orifice_diameter": 0.3,
        "flow_coefficient": 0.62,
    },
    {
        "id": "KD3",
        "name": "平水壶",
        "max_level": 80.0,
        "min_level": 10.0,
        "init_level": 65.0,
        "base_flow": 2.5,
        "cross_section": 78.54,
        "orifice_diameter": 0.3,
        "flow_coefficient": 0.62,
    },
    {
        "id": "KD4",
        "name": "万分水",
        "max_level": 60.0,
        "min_level": 5.0,
        "init_level": 50.0,
        "base_flow": 2.5,
        "cross_section": 78.54,
        "orifice_diameter": 0.3,
        "flow_coefficient": 0.62,
    },
]

GRAVITY = 980.665
STANDARD_PRESSURE = 101.325


class ClepsydraSimulator:
    def __init__(
        self,
        config,
        start_time=None,
        altitude_m=0,
        base_temp=20.0,
        base_pressure=None,
        temp_variation=5.0,
    ):
        self.config = config
        self.water_level = config["init_level"]
        self.base_temp = base_temp
        self.water_temp = base_temp
        self.humidity = 60.0
        self.quality = 1.0
        self.flow_rate = config["base_flow"]
        self.last_time = start_time or time.time()
        self.inflow = config["base_flow"] * 1.05
        self.day_phase = 0.0
        self.temp_variation = temp_variation

        if base_pressure is not None:
            self.base_pressure = base_pressure
        else:
            self.base_pressure = STANDARD_PRESSURE * math.pow(
                1.0 - 2.25577e-5 * altitude_m, 5.25588
            )
        self.pressure = self.base_pressure

        self.abnormal_active = False
        self.abnormal_timer = 0.0
        self.abnormal_target_level = None
        self.abnormal_duration = 0.0

    def viscosity_correction(self, temp_c):
        t = max(0.0, min(100.0, temp_c))
        nu = 1.792e-2 / (1.0 + 0.0337 * t + 0.000221 * t * t)
        nu_ref = 1.308e-2
        return math.pow(nu_ref / nu, 0.1)

    def calculate_flow(self):
        head = self.water_level / 10.0
        velocity = math.sqrt(2 * GRAVITY * head)
        orifice_area = math.pi * (self.config["orifice_diameter"] / 2.0) ** 2
        viscosity_factor = self.viscosity_correction(self.water_temp)
        flow = self.config["flow_coefficient"] * orifice_area * velocity * viscosity_factor
        flow *= self.quality
        noise = random.gauss(0, flow * 0.02)
        return max(0.01, flow + noise)

    def calculate_evaporation(self, dt):
        svp = 610.78 * math.exp((17.27 * self.water_temp) / (self.water_temp + 237.3))
        avp = svp * (self.humidity / 100.0)
        pressure_diff = svp - avp
        t_kelvin = self.water_temp + 273.15
        mass_flux = 0.001 * pressure_diff / math.sqrt(t_kelvin)
        surface_area = self.config["cross_section"]
        volume_flux = mass_flux * surface_area * self.quality / 1000.0
        return volume_flux * dt

    def inject_abnormal_water_level(self, target_level, duration=60):
        """注入水位异常"""
        self.abnormal_active = True
        self.abnormal_target_level = target_level
        self.abnormal_duration = duration
        self.abnormal_timer = 0.0
        print(f"  ⚠️  {self.config['name']} 注入水位异常: {target_level:.1f}cm, 持续 {duration}s")

    def update(self, dt):
        self.day_phase += dt / 86400.0
        if self.day_phase > 1.0:
            self.day_phase -= 1.0

        temp_variation = self.temp_variation * math.sin(2 * math.pi * (self.day_phase - 0.25))
        self.water_temp = self.base_temp + temp_variation + random.gauss(0, 0.3)

        humidity_variation = 15.0 * math.sin(2 * math.pi * (self.day_phase - 0.5))
        self.humidity = 60.0 + humidity_variation + random.gauss(0, 1.0)
        self.humidity = max(30.0, min(90.0, self.humidity))

        self.quality = 1.0 + 0.05 * math.sin(self.day_phase * 2 * math.pi * 3)
        self.quality += random.gauss(0, 0.02)
        self.quality = max(0.8, min(1.2, self.quality))

        pressure_variation = 0.3 * math.sin(2 * math.pi * (self.day_phase - 0.3))
        self.pressure = self.base_pressure + pressure_variation + random.gauss(0, 0.05)
        self.pressure = max(50.0, min(150.0, self.pressure))

        self.flow_rate = self.calculate_flow()

        outflow = self.flow_rate
        evaporation = self.calculate_evaporation(dt)
        net_volume_change = (self.inflow - outflow) * dt - evaporation
        level_change = net_volume_change / self.config["cross_section"]

        self.water_level += level_change
        self.water_level = max(
            self.config["min_level"], min(self.config["max_level"], self.water_level)
        )

        if self.abnormal_active:
            self.abnormal_timer += dt
            if self.abnormal_timer < self.abnormal_duration:
                target = self.abnormal_target_level
                self.water_level += (target - self.water_level) * 0.2
            else:
                self.abnormal_active = False
                self.abnormal_timer = 0.0
                self.abnormal_target_level = None
                print(f"  ✓ {self.config['name']} 水位异常结束，恢复正常")

        if self.water_level <= self.config["min_level"] + 1.0:
            self.inflow = self.config["base_flow"] * 1.2
        elif self.water_level >= self.config["max_level"] - 5.0:
            self.inflow = self.config["base_flow"] * 0.9
        else:
            self.inflow = self.config["base_flow"] * (
                1.0 + 0.1 * math.sin(self.day_phase * 2 * math.pi)
            )

    def get_sensor_data(self):
        return {
            "water_level": round(self.water_level, 3),
            "flow_rate": round(self.flow_rate, 4),
            "water_temp": round(self.water_temp, 2),
            "humidity": round(self.humidity, 1),
            "quality": round(self.quality, 3),
            "pressure": round(self.pressure, 3),
            "timestamp": int(time.time() * 1000),
        }


def on_connect(client, userdata, flags, rc):
    if rc == 0:
        print("✅ MQTT连接成功")
    else:
        print(f"❌ MQTT连接失败，错误码: {rc}")


def publish_sensor_data(client, topic_prefix, simulators):
    for sim in simulators:
        topic = f"{topic_prefix}/{sim.config['id']}"
        data = sim.get_sensor_data()
        payload = json.dumps(data)
        client.publish(topic, payload, qos=1)
        print(
            f"[{datetime.now().strftime('%H:%M:%S')}] "
            f"{sim.config['name']}({sim.config['id']}): "
            f"水位={data['water_level']:.2f}cm, "
            f"流量={data['flow_rate']:.4f}mL/s, "
            f"水温={data['water_temp']:.1f}°C, "
            f"气压={data['pressure']:.2f}kPa"
        )


def parse_args():
    parser = argparse.ArgumentParser(
        description="漏壶传感器模拟器（增强版）",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
示例:
  # 默认参数运行
  python simulator.py

  # 模拟高海拔（拉萨 3650m）
  python simulator.py --altitude 3650

  # 模拟高温环境（35°C 基准水温）
  python simulator.py --water-temp 35

  # 设定固定气压（95kPa）
  python simulator.py --pressure 95

  # 每隔120秒在平水壶注入低水位异常
  python simulator.py --abnormal --abnormal-interval 120 --abnormal-target KD3 --abnormal-level 15

  # 同时模拟高温+低气压+水位异常
  python simulator.py --water-temp 30 --pressure 90 --abnormal --abnormal-interval 60
        """,
    )
    parser.add_argument("--broker", default=os.getenv("MQTT_BROKER", "localhost"), help="MQTT broker地址")
    parser.add_argument("--port", type=int, default=int(os.getenv("MQTT_PORT", "1883")), help="MQTT端口")
    parser.add_argument("--topic", default=os.getenv("MQTT_TOPIC", "clepsydra/sensor"), help="MQTT主题前缀")
    parser.add_argument("--interval", type=float, default=float(os.getenv("SIM_INTERVAL", "1.0")), help="上报间隔（秒）")
    parser.add_argument("--simulate-days", type=float, default=0, help="加速模拟天数（0为实时）")

    parser.add_argument("--altitude", type=float, default=float(os.getenv("SIM_ALTITUDE", "0")), help="海拔高度（米），用于计算基准气压")
    parser.add_argument("--water-temp", type=float, default=float(os.getenv("SIM_WATER_TEMP", "20.0")), help="基准水温（°C）")
    parser.add_argument("--pressure", type=float, default=None, help="固定基准气压（kPa），会覆盖--altitude计算值")
    parser.add_argument("--temp-variation", type=float, default=5.0, help="水温日变化幅度（°C）")

    parser.add_argument(
        "--abnormal",
        action="store_true",
        default=os.getenv("SIM_ABNORMAL_WATER_LEVEL", "false").lower() == "true",
        help="启用水位异常注入",
    )
    parser.add_argument(
        "--abnormal-interval",
        type=int,
        default=int(os.getenv("SIM_ABNORMAL_INTERVAL", "300")),
        help="水位异常注入间隔（秒）",
    )
    parser.add_argument(
        "--abnormal-target",
        default=os.getenv("SIM_ABNORMAL_TARGET", "random"),
        help="异常目标漏壶ID（KD1/KD2/KD3/KD4/random）",
    )
    parser.add_argument(
        "--abnormal-level",
        type=float,
        default=float(os.getenv("SIM_ABNORMAL_LEVEL", "10.0")),
        help="异常水位值（cm）",
    )
    parser.add_argument(
        "--abnormal-duration",
        type=int,
        default=int(os.getenv("SIM_ABNORMAL_DURATION", "60")),
        help="异常持续时间（秒）",
    )
    parser.add_argument(
        "--abnormal-type",
        choices=["low", "high", "random"],
        default=os.getenv("SIM_ABNORMAL_TYPE", "low"),
        help="异常类型: low(低水位), high(高水位), random(随机)",
    )

    return parser.parse_args()


def main():
    args = parse_args()

    print("=" * 70)
    print("  古代水运仪象台 - 漏壶传感器模拟器（增强版）")
    print("=" * 70)
    print(f"Broker:      {args.broker}:{args.port}")
    print(f"Topic:       {args.topic}/<漏壶ID>")
    print(f"间隔:        {args.interval}秒")
    print(f"海拔:        {args.altitude}m")
    print(f"基准水温:    {args.water_temp}°C")
    print(f"基准气压:    {args.pressure or '由海拔计算'} kPa")
    print(f"水位异常:    {'启用' if args.abnormal else '禁用'}")
    if args.abnormal:
        print(f"  目标:      {args.abnormal_target}")
        print(f"  类型:      {args.abnormal_type}")
        print(f"  间隔:      {args.abnormal_interval}s")
        print(f"  持续:      {args.abnormal_duration}s")
        print(f"  异常水位:  {args.abnormal_level}cm")
    print(f"模拟四级漏壶: KD1天上壶, KD2夜漏壶, KD3平水壶, KD4万分水")
    print("=" * 70)

    client = mqtt.Client(client_id=f"clepsydra-simulator-{int(time.time())}")
    client.on_connect = on_connect

    try:
        client.connect(args.broker, args.port, keepalive=60)
    except Exception as e:
        print(f"❌ 无法连接MQTT broker: {e}")
        print("请确保MQTT服务器已启动，或使用 --broker 指定正确地址")
        sys.exit(1)

    client.loop_start()

    base_pressure = args.pressure
    simulators = [
        ClepsydraSimulator(
            cfg,
            altitude_m=args.altitude,
            base_temp=args.water_temp,
            base_pressure=base_pressure,
            temp_variation=args.temp_variation,
        )
        for cfg in CLEPSYDRAS
    ]

    abnormal_counter = 0

    try:
        print("\n🚀 开始发送传感器数据... (Ctrl+C 停止)\n")
        while True:
            dt = args.interval
            if args.simulate_days > 0:
                dt = args.interval * 86400 / 1000

            for sim in simulators:
                sim.update(dt)

            publish_sensor_data(client, args.topic, simulators)

            if args.abnormal:
                abnormal_counter += args.interval
                if abnormal_counter >= args.abnormal_interval:
                    abnormal_counter = 0

                    if args.abnormal_target == "random":
                        target_sim = random.choice(simulators)
                    else:
                        target_sim = next(
                            (s for s in simulators if s.config["id"] == args.abnormal_target),
                            None,
                        )
                        if target_sim is None:
                            print(f"⚠️  未找到目标漏壶: {args.abnormal_target}")
                            target_sim = random.choice(simulators)

                    abnormal_type = args.abnormal_type
                    if abnormal_type == "random":
                        abnormal_type = random.choice(["low", "high"])

                    if abnormal_type == "low":
                        level = args.abnormal_level
                    else:
                        level = target_sim.config["max_level"] - args.abnormal_level

                    target_sim.inject_abnormal_water_level(
                        level, duration=args.abnormal_duration
                    )

            time.sleep(args.interval)

    except KeyboardInterrupt:
        print("\n\n🛑 模拟器已停止")
    finally:
        client.loop_stop()
        client.disconnect()


if __name__ == "__main__":
    main()
