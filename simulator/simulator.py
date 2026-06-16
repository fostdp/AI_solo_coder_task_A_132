#!/usr/bin/env python3
"""
古代水运仪象台漏壶传感器模拟器
模拟四级漏壶（天上壶、夜漏壶、平水壶、万分水）的传感器数据
每秒通过MQTT上报水位、流量、水温、环境湿度、水质
"""

import json
import time
import random
import math
import argparse
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


class ClepsydraSimulator:
    def __init__(self, config, start_time=None):
        self.config = config
        self.water_level = config["init_level"]
        self.water_temp = 20.0
        self.humidity = 60.0
        self.quality = 1.0
        self.flow_rate = config["base_flow"]
        self.last_time = start_time or time.time()
        self.inflow = config["base_flow"] * 1.05
        self.day_phase = 0.0

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

    def update(self, dt):
        self.day_phase += dt / 86400.0
        if self.day_phase > 1.0:
            self.day_phase -= 1.0

        temp_variation = 5.0 * math.sin(2 * math.pi * (self.day_phase - 0.25))
        self.water_temp = 20.0 + temp_variation + random.gauss(0, 0.3)

        humidity_variation = 15.0 * math.sin(2 * math.pi * (self.day_phase - 0.5))
        self.humidity = 60.0 + humidity_variation + random.gauss(0, 1.0)
        self.humidity = max(30.0, min(90.0, self.humidity))

        self.quality = 1.0 + 0.05 * math.sin(self.day_phase * 2 * math.pi * 3)
        self.quality += random.gauss(0, 0.02)
        self.quality = max(0.8, min(1.2, self.quality))

        self.flow_rate = self.calculate_flow()

        outflow = self.flow_rate
        evaporation = self.calculate_evaporation(dt)
        net_volume_change = (self.inflow - outflow) * dt - evaporation
        level_change = net_volume_change / self.config["cross_section"]

        self.water_level += level_change
        self.water_level = max(self.config["min_level"], min(self.config["max_level"], self.water_level))

        if self.water_level <= self.config["min_level"] + 1.0:
            self.inflow = self.config["base_flow"] * 1.2
        elif self.water_level >= self.config["max_level"] - 5.0:
            self.inflow = self.config["base_flow"] * 0.9
        else:
            self.inflow = self.config["base_flow"] * (1.0 + 0.1 * math.sin(self.day_phase * 2 * math.pi))

    def get_sensor_data(self):
        return {
            "water_level": round(self.water_level, 3),
            "flow_rate": round(self.flow_rate, 4),
            "water_temp": round(self.water_temp, 2),
            "humidity": round(self.humidity, 1),
            "quality": round(self.quality, 3),
            "timestamp": int(time.time() * 1000),
        }


def on_connect(client, userdata, flags, rc):
    if rc == 0:
        print("MQTT连接成功")
    else:
        print(f"MQTT连接失败，错误码: {rc}")


def publish_sensor_data(client, topic_prefix, simulators):
    for sim in simulators:
        topic = f"{topic_prefix}/{sim.config['id']}"
        data = sim.get_sensor_data()
        payload = json.dumps(data)
        client.publish(topic, payload, qos=1)
        print(f"[{datetime.now().strftime('%H:%M:%S')}] {sim.config['name']}({sim.config['id']}): "
              f"水位={data['water_level']:.2f}cm, 流量={data['flow_rate']:.4f}mL/s, "
              f"水温={data['water_temp']:.1f}°C, 湿度={data['humidity']:.1f}%")


def main():
    parser = argparse.ArgumentParser(description="漏壶传感器模拟器")
    parser.add_argument("--broker", default="localhost", help="MQTT broker地址")
    parser.add_argument("--port", type=int, default=1883, help="MQTT端口")
    parser.add_argument("--topic", default="clepsydra/sensor", help="MQTT主题前缀")
    parser.add_argument("--interval", type=float, default=1.0, help="上报间隔（秒）")
    parser.add_argument("--simulate-days", type=float, default=0, help="加速模拟天数（0为实时）")
    args = parser.parse_args()

    print("=" * 60)
    print("  古代水运仪象台 - 漏壶传感器模拟器")
    print("=" * 60)
    print(f"Broker: {args.broker}:{args.port}")
    print(f"Topic: {args.topic}/<漏壶ID>")
    print(f"间隔: {args.interval}秒")
    print(f"模拟四级漏壶: KD1天上壶, KD2夜漏壶, KD3平水壶, KD4万分水")
    print("=" * 60)

    client = mqtt.Client(client_id=f"clepsydra-simulator-{int(time.time())}")
    client.on_connect = on_connect

    try:
        client.connect(args.broker, args.port, keepalive=60)
    except Exception as e:
        print(f"无法连接MQTT broker: {e}")
        print("请确保MQTT服务器已启动，或使用 --broker 指定正确地址")
        exit(1)

    client.loop_start()

    simulators = [ClepsydraSimulator(cfg) for cfg in CLEPSYDRAS]

    try:
        print("\n开始发送传感器数据... (Ctrl+C 停止)\n")
        while True:
            dt = args.interval
            if args.simulate_days > 0:
                dt = args.interval * 86400 / 1000

            for sim in simulators:
                sim.update(dt)

            publish_sensor_data(client, args.topic, simulators)
            time.sleep(args.interval)

    except KeyboardInterrupt:
        print("\n\n模拟器已停止")
    finally:
        client.loop_stop()
        client.disconnect()


if __name__ == "__main__":
    main()
