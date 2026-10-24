# Multi-stage production build for Sandsifter MCP & Outlier Daemon
# Target platform: Google Cloud Run / Linux x86_64

# Stage 1: Rust Builder
FROM rust:1.84-bookworm AS builder
WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
# Create dummy main to compile dependencies first
RUN mkdir -p src && echo "fn main() {}" > src/main.rs && echo "pub mod lib;" > src/lib.rs && touch src/lib.rs
RUN cargo build --release || true
RUN rm -rf src

# Copy real source code
COPY src ./src
COPY tests ./tests
RUN cargo build --release

# Stage 2: Minimal Runtime Image
FROM debian:bookworm-slim AS runtime

# Install CA certificates, curl, yt-dlp, and python3 for stream ingestion
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    python3 \
    python3-pip \
    ffmpeg \
    && rm -rf /var/lib/apt/lists/*

# Install yt-dlp directly for YouTube Shorts & TikTok stream extraction
RUN curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp \
    && chmod a+rx /usr/local/bin/yt-dlp

WORKDIR /app

# Copy compiled binary from builder
COPY --from=builder /app/target/release/sandsifter /usr/local/bin/sandsifter

# Default Cloud Run environment variables
ENV HOST="0.0.0.0"
ENV PORT="8080"
ENV RUST_LOG="info"

EXPOSE 8080

# Cloud Run execution entrypoint
ENTRYPOINT ["/usr/local/bin/sandsifter"]
CMD ["--sse", "8080"]
