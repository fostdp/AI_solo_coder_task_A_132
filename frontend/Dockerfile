FROM nginx:1.27-alpine

RUN apk add --no-cache tzdata ca-certificates

ENV TZ=Asia/Shanghai

COPY nginx.conf /etc/nginx/conf.d/default.conf

COPY index.html /usr/share/nginx/html/index.html
COPY js /usr/share/nginx/html/js
COPY src /usr/share/nginx/html/src

EXPOSE 80

HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
    CMD wget -qO- http://localhost/ || exit 1
