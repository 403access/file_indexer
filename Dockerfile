FROM rust:1.90-alpine

RUN apk add --no-cache musl-dev

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

CMD ["cargo", "test"]
