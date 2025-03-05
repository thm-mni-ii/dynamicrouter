FROM rust:1.84.0 AS build
WORKDIR /build
RUN cargo install sqlx-cli
COPY Cargo.toml Cargo.lock /build/
COPY src/ /build/src/
COPY migrations/ /build/migrations/
ENV DATABASE_URL=sqlite:build.db
RUN touch build.db && sqlx migrate run
RUN cargo build --release

FROM debian:12
RUN apt-get update && apt-get install -y sqlite3 && rm -rf rm -rf /var/lib/apt/lists/*
COPY --from=build /build/target/release/dynamicrouter /usr/local/bin/
CMD ["/usr/local/bin/dynamicrouter"]
