# Multi-stage production build for Gafana (Rust Edition)
FROM rust:alpine AS builder
RUN apk add --no-cache musl-dev

WORKDIR /build
COPY Cargo.toml ./
COPY src/ ./src/
COPY examples/ ./examples/
COPY web/ ./web/

# Compile static release binaries
RUN cargo build --release

# Final Minimal Container
FROM alpine:3.19
WORKDIR /app
COPY --from=builder /build/target/release/gafana /app/gafana
COPY --from=builder /build/target/release/gafana-relay /app/gafana-relay
COPY web/ /app/web/

# Expose UDP relay port and HTTP web port
EXPOSE 8765/udp
EXPOSE 7878/tcp

ENTRYPOINT ["/app/gafana"]
