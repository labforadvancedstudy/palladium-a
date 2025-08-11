# Build stage
FROM rust:1.89 AS builder

WORKDIR /usr/src/palladium

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage
FROM ubuntu:22.04

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y \
        gcc \
        libc6-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary
COPY --from=builder /usr/src/palladium/target/release/palladium /usr/local/bin/palladium

# Copy bootstrap compilers and examples
COPY bootstrap /opt/palladium/bootstrap
COPY examples /opt/palladium/examples
COPY docs /opt/palladium/docs

# Set working directory
WORKDIR /workspace

# Add palladium to PATH
ENV PATH="/usr/local/bin:${PATH}"

# Verify installation
RUN palladium --version

# Default command
CMD ["palladium", "--help"]