# Build stage
FROM rust:1.75 as builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml ./
COPY migration/Cargo.toml ./migration/

# Copy source code
COPY src ./src
COPY migration ./migration

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/asterdrive /app/

# Create data directory
RUN mkdir -p /app/data/uploads

# Expose port
EXPOSE 3000

# Run the application
CMD ["/app/asterdrive"]
