# Dev image for the Rust core + devserver (docker compose `back`).
# Source is bind-mounted; this image only provides the toolchain.
# `rust:1` tracks stable on purpose, matching CI (dtolnay/rust-toolchain@stable).
FROM rust:1-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends curl sqlite3 \
    && rm -rf /var/lib/apt/lists/* \
    && rustup component add rustfmt clippy \
    && cargo install cargo-watch --version 8.5.3 --locked

WORKDIR /workspace
ENV CARGO_TARGET_DIR=/cargo-target \
    MRK_BIND=0.0.0.0:7878 \
    MRK_DB_PATH=/data/mrkrabs.sqlite \
    RUST_LOG=info,tower_http=info

EXPOSE 7878
# No HEALTHCHECK: compose owns it (compose.yaml, service `back`).
# --locked, as in CI: the repo is bind-mounted, so cargo must not rewrite Cargo.lock.
CMD ["cargo", "watch", "--poll", "-w", "crates", "-w", "apps/devserver", "-x", "run --locked -p mrk-devserver"]
