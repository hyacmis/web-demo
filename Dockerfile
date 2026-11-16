FROM rust:1.100-slim-bookworm AS builder

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY ./build/config /root/.cargo/config

COPY . .

RUN cargo install --path .


FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

RUN useradd --system --uid 10001 --create-home app

COPY --from=builder /usr/local/cargo/bin/web-demo /usr/local/bin/web-demo
COPY config.toml.example ./config.toml

RUN chown -R app:app /app

ENV WEB_DEMO_APP__HOST=0.0.0.0
EXPOSE 8000
USER app

ENTRYPOINT ["web-demo"]
