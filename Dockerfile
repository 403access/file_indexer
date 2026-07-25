FROM rust:1.90

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

CMD ["cargo", "test"]
