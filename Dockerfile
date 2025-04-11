# Stage 1: Build the application
# Pin base image versions for reproducible builds (use recent stable versions)
FROM rust:1.86.0-bookworm AS builder
# ARG CARGO_PROFILE=release # Default to release, can be overridden

WORKDIR /usr/src/app

# Create a non-root user and group first for consistent ownership
# Using fixed IDs > 1000 is good practice
RUN groupadd --system --gid 1001 appgroup && \
    useradd --system --uid 1001 --gid appgroup appuser

# Install build-time dependencies if needed (e.g., for specific crates)
# Example: RUN apt-get update && apt-get install -y --no-install-recommends protobuf-compiler libssl-dev pkg-config && rm -rf /var/lib/apt/lists/*

# Copy manifests and source code with correct ownership
COPY --chown=appuser:appgroup Cargo.toml Cargo.lock ./

# Copy source code *before* the build
COPY --chown=appuser:appgroup src ./src

# Build the final application executable
# This single command handles both dependencies and compilation
RUN cargo build --release --locked --bin axum_meme_posting_example

# Stage 2: Create the minimal runtime image
# Pin Debian version and use slim variant
FROM debian:12.10-slim

# Create the same non-root user and group
RUN groupadd --system --gid 1001 appgroup && \
    useradd --system --uid 1001 --gid appgroup appuser

# Install runtime dependencies:
# - ca-certificates: For TLS/SSL verification (connecting to AWS/HTTPS)
# - curl: Used for the HEALTHCHECK command
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl && \
    # Clean up apt caches to reduce image size
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy the built application executable from the builder stage with correct ownership
COPY --from=builder --chown=appuser:appgroup /usr/src/app/target/release/axum_meme_posting_example .

# Switch to the non-root user
USER appuser

# Expose the port the application listens on (must match APP_SERVER_ADDRESS in compose)
EXPOSE 3000

# Healthcheck: Ensure API is available.
HEALTHCHECK --interval=15s --timeout=5s --start-period=30s --retries=3 \
  CMD curl --fail http://localhost:3000/health || exit 1
  # Alternative using wget: CMD wget --quiet --tries=1 --spider http://localhost:3000/health || exit 1

# Command to run the application when the container starts
CMD ["./axum_meme_posting_example"]