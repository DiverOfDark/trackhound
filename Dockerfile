FROM rust:1.95-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/trackhound /usr/local/bin/trackhound
EXPOSE 8080
VOLUME ["/data"]
ENTRYPOINT ["/usr/local/bin/trackhound"]
CMD ["serve"]
