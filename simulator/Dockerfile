FROM python:3.12-alpine3.20

RUN apk add --no-cache \
    ca-certificates \
    tzdata \
    && pip install --no-cache-dir paho-mqtt

ENV TZ=Asia/Shanghai \
    PYTHONUNBUFFERED=1

WORKDIR /app

COPY simulator.py /app/simulator.py

RUN chmod +x /app/simulator.py

ENTRYPOINT ["python", "/app/simulator.py"]
CMD ["--broker", "emqx", "--interval", "1"]
