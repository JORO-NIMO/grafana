# Multi-stage production build for Gafana Stateless SFU Relay
FROM alpine:3.19 AS builder
RUN apk add --no-cache build-base

WORKDIR /build
COPY include/ ./include/
COPY src/ ./src/
COPY server/ ./server/

# Compile static relay binary with maximum optimization
RUN gcc -O3 -Wall -Wextra -static server/gafana_sfu.c -o gafana-relay

# Final Minimal Distroless Image
FROM scratch
WORKDIR /app
COPY --from=builder /build/gafana-relay /app/gafana-relay

# UDP Port for stateless packet relay
EXPOSE 8765/udp

USER 10001:10001
ENTRYPOINT ["/app/gafana-relay", "8765"]
