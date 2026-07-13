FROM rust:1.95-alpine AS builder

RUN apk add --no-cache \
    build-base \
    musl-dev

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release --locked

FROM alpine:3.24

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/release/mealie /usr/local/bin/mealie

ENTRYPOINT ["mealie"]
