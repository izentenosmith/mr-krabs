# Dev image for the Rust core + devserver (docker compose `back`).
# Source is bind-mounted; this image only provides the toolchain.
FROM rust:1-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends curl sqlite3 \
    && rm -rf /var/lib/apt/lists/* \
    && rustup component add rustfmt clippy \
    && cargo install cargo-watch --locked

WORKDIR /workspace
ENV CARGO_TARGET_DIR=/cargo-target \
    MRK_BIND=0.0.0.0:7878 \
    MRK_DB_PATH=/data/mrkrabs.sqlite \
    RUST_LOG=info,tower_http=info

EXPOSE 7878
CMD ["cargo", "watch", "--poll", "-w", "crates", "-w", "apps/devserver", "-x", "run -p mrk-devserver"]
