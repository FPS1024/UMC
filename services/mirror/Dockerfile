# syntax=docker/dockerfile:1

FROM golang:1.24-alpine AS builder
WORKDIR /src
COPY go.mod ./
COPY cmd ./cmd
RUN CGO_ENABLED=0 GOOS=linux go build -trimpath -ldflags="-s -w" -o /out/umc-mirror ./cmd/umc-mirror

FROM alpine:3.21
RUN addgroup -g 10001 umc \
    && adduser -D -u 10001 -G umc -h /data umc \
    && mkdir -p /data/cache \
    && chown -R 10001:10001 /data \
    && apk add --no-cache ca-certificates wget
COPY --from=builder /out/umc-mirror /usr/local/bin/umc-mirror
USER 10001
ENV BIND_ADDR=0.0.0.0:8080 \
    CACHE_DIR=/data/cache \
    PROVIDERS_FILE=/etc/umc-mirror/providers.json \
    MAX_OBJECT_BYTES=536870912 \
    CACHE_MAX_BYTES=10737418240 \
    MAX_CONCURRENT_DOWNLOADS=4
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD wget -q -O /dev/null http://127.0.0.1:8080/healthz || exit 1
ENTRYPOINT ["umc-mirror"]
