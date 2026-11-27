# syntax=docker/dockerfile:1
FROM rust:alpine AS chef
RUN apk add --no-cache build-base
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install cargo-chef --locked
WORKDIR /app
ENV SQLX_OFFLINE=true

FROM chef AS planner
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Dependencies only: cached as a layer until Cargo.toml/Cargo.lock change.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo chef cook --release --locked --recipe-path recipe.json
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY migrations ./migrations
COPY .sqlx ./.sqlx
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release --locked

FROM alpine:3 AS runtime
RUN apk add --no-cache ca-certificates \
    && adduser -D -H -u 10001 ding \
    && mkdir /data && chown ding /data
COPY --from=builder /app/target/release/ding /usr/local/bin/ding
USER ding
WORKDIR /data
ENV DATABASE_URL=sqlite:/data/ding.db
VOLUME /data
ENTRYPOINT ["ding"]
