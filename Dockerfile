FROM rust:1.91-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY assets ./assets
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/typst-test /usr/local/bin/typst-service
COPY assets ./assets
ENV APP_BIND=0.0.0.0:3000
ENV APP_STORAGE_ROOT=/data
ENV APP_API_KEYS=dev-secret
EXPOSE 3000
CMD ["typst-service"]
