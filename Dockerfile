# --- Build stage ---
FROM rust:1.85-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
# Create a dummy main to cache dependency compilation
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release 2>/dev/null || true
# Now copy real source and rebuild
COPY src/ src/
COPY migrations/ migrations/
RUN cargo build --release

# --- Runtime stage ---
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/homorg /app/homorg
COPY migrations/ /app/migrations/
EXPOSE 8080
ENTRYPOINT ["/app/homorg"]
