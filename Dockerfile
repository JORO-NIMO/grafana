# Multi-Stage Production Build for Safe Spaces AI Innovation Hub Grafana
FROM rust:1.80-alpine AS builder

WORKDIR /usr/src/grafana
RUN apk add --no-cache musl-dev

# Copy project files
COPY Cargo.toml ./
COPY src/ ./src/
COPY web/ ./web/
COPY logo.png ./

# Compile release binary with full optimizations
RUN cargo build --release

# Minimal scratch/alpine runtime container
FROM alpine:3.20

WORKDIR /app
RUN apk add --no-cache curl ca-certificates tzdata

# Copy binary and assets
COPY --from=builder /usr/src/grafana/target/release/grafana /app/grafana
COPY web/ /app/web/
COPY logo.png /app/logo.png
RUN mkdir -p /app/data

ENV GRAFANA_PORT=7878
EXPOSE 7878 50000-50100/udp

ENTRYPOINT ["/app/grafana"]
