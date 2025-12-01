FROM rust:1.90.0 AS build
RUN cargo install sqlx-cli trunk
RUN rustup target add wasm32-unknown-unknown
COPY Cargo.toml Cargo.lock /build/
COPY src/ /build/src/
COPY migrations/ /build/migrations/
COPY ui/Cargo.toml ui/Cargo.lock ui/index.html ui/pico.lime.min.css /build/ui/
COPY ui/src/ /build/ui/src/
WORKDIR /build/ui
RUN trunk build --release --filehash false
WORKDIR /build
ENV DATABASE_URL=sqlite:build.db
RUN touch build.db && sqlx migrate run
RUN cargo build --release

FROM debian:13
RUN apt-get update && apt-get install -y sqlite3 && rm -rf rm -rf /var/lib/apt/lists/*
COPY --from=build /build/target/release/dynamicrouter /usr/local/bin/
CMD ["/usr/local/bin/dynamicrouter"]
