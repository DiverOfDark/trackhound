FROM rust:1.95-bookworm AS builder
WORKDIR /app

# Build dependency graph first so Docker can reuse this layer when only source
# files change. The dummy main keeps Cargo happy while compiling dependencies.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
    && printf 'fn main() {}\n' > src/main.rs \
    && cargo build --release \
    && rm -rf src target/release/deps/trackhound* target/release/trackhound*

COPY src ./src
COPY migrations ./migrations
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/trackhound /usr/local/bin/trackhound
EXPOSE 8080
VOLUME ["/data"]
ENTRYPOINT ["/usr/local/bin/trackhound"]
CMD ["serve"]
