FROM rust:1-alpine3.19 AS builder

RUN apk add --no-cache musl-dev gcc

RUN cargo new --bin app
WORKDIR /app

COPY Cargo.toml Cargo.lock /app/
RUN cargo fetch

COPY src /app/src
RUN touch /app/src/main.rs
RUN cargo build --release

FROM alpine:3.19

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/release/rinha-backend-2024-q1-rust /app/rinha

CMD ["/app/rinha"]