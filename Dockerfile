FROM rust:1.91-bookworm AS chef
WORKDIR /app
RUN cargo install cargo-chef --locked

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY assets ./assets
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY assets ./assets
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    fontconfig \
    fonts-noto-cjk \
    fonts-liberation \
 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/typst-pdf-service /usr/local/bin/typst-pdf-service
COPY assets ./assets
ENV APP_BIND=0.0.0.0:3000
ENV APP_STORAGE_ROOT=/data
ENV APP_API_KEYS=dev-secret
EXPOSE 3000
CMD ["typst-pdf-service"]
