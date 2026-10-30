# syntax=docker/dockerfile:1

# Build stage: compiled by the image's stable toolchain (see rust-toolchain.toml).
FROM rust:bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release --locked -p framefeed-cli

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ffmpeg ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home framefeed

COPY --from=build /src/target/release/framefeed /usr/local/bin/framefeed
ENV FRAMEFEED_FFMPEG=/usr/bin/ffmpeg \
    FRAMEFEED_FFPROBE=/usr/bin/ffprobe

USER framefeed
EXPOSE 8080

# Public sources only: --allow-local is deliberately absent from the image.
ENTRYPOINT ["framefeed", "serve", "--listen", "0.0.0.0:8080"]
