# syntax=docker/dockerfile:1
ARG RUST_VERSION=1.94.0
ARG BUILD_ENV=local
ARG CARGO_CHEF_VERSION=0.1.77

# 1. Base image with cargo-chef
# For production: pin with @sha256:<digest> after RUST_VERSION for reproducibility.
# Template keeps tag-only so RUST_VERSION ARG remains functional.
FROM lukemathwalker/cargo-chef:latest-rust-${RUST_VERSION} AS chef
ARG CARGO_CHEF_VERSION
WORKDIR /workspace
# CARGO_TARGET_DIR is placed outside the /workspace bind-mount so that a named
# Docker volume at /cargo-target is not shadowed by the host bind-mount.
# This prevents GHA from getting an empty /cargo-target (which would force a full
# dependency rebuild) even though the image already has cook-baked deps there.
ENV CARGO_HOME=/usr/local/cargo \
    CARGO_TARGET_DIR=/cargo-target \
    PATH=/usr/local/cargo/bin:$PATH

ARG UV_VERSION=0.7.12

RUN apt-get update && apt-get install -y \
    mold \
    clang \
    cmake \
    curl \
    python3 \
    python3-yaml \
    python3-pytest \
    ripgrep \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/* \
    && rustup component add --toolchain ${RUST_VERSION} rustfmt clippy \
    && curl -LsSf https://astral.sh/uv/${UV_VERSION}/install.sh | sh

RUN --mount=type=cache,target=${CARGO_HOME}/registry,sharing=locked \
    --mount=type=cache,target=${CARGO_HOME}/git,sharing=locked \
    --mount=type=cache,target=/cargo-target,sharing=locked \
    cargo install --locked cargo-chef --version ${CARGO_CHEF_VERSION} --force

# Python deps from requirements-python.txt (SSoT for ruff version).
# Placed after apt-get and cargo-chef layers so only this small layer
# rebuilds when Python deps change.
COPY requirements-python.txt /tmp/requirements-python.txt
RUN $HOME/.local/bin/uv pip install --system --break-system-packages \
    -r /tmp/requirements-python.txt

# 2. Build tool binaries once
FROM chef AS tools-builder
ARG SCCACHE_VERSION=0.14.0
ARG BACON_VERSION=3.22.0
ARG CARGO_BINSTALL_VERSION=1.17.6
ARG CARGO_MAKE_VERSION=0.37.24
ARG CARGO_DENY_VERSION=0.19.0
ARG CARGO_MACHETE_VERSION=0.9.1
ARG CARGO_NEXTEST_VERSION=0.9.129
ARG CARGO_LLVM_COV_VERSION=0.8.4

RUN --mount=type=cache,target=${CARGO_HOME}/registry,sharing=locked \
    --mount=type=cache,target=/opt/sccache,sharing=shared \
    cargo install --locked cargo-binstall --version ${CARGO_BINSTALL_VERSION} && \
    cargo binstall -y --root /usr/local/cargo sccache@${SCCACHE_VERSION} && \
    cargo binstall -y --root /usr/local/cargo bacon@${BACON_VERSION} && \
    cargo binstall -y --root /usr/local/cargo cargo-make@${CARGO_MAKE_VERSION} && \
    cargo binstall -y --root /usr/local/cargo cargo-deny@${CARGO_DENY_VERSION} && \
    cargo binstall -y --root /usr/local/cargo cargo-machete@${CARGO_MACHETE_VERSION} && \
    cargo binstall -y --root /usr/local/cargo cargo-nextest@${CARGO_NEXTEST_VERSION} && \
    cargo binstall -y --root /usr/local/cargo cargo-llvm-cov@${CARGO_LLVM_COV_VERSION}

# 3. Generate dependency recipe
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# 4. Release dependency build base
FROM chef AS builder-base
COPY --from=tools-builder /usr/local/cargo/bin/sccache /usr/local/cargo/bin/sccache
ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache \
    SCCACHE_DIR=/opt/sccache \
    SCCACHE_IDLE_TIMEOUT=600
COPY --from=planner /workspace/recipe.json recipe.json
COPY vendor/ vendor/
RUN --mount=type=cache,target=${CARGO_HOME}/registry,sharing=locked \
    --mount=type=cache,target=${CARGO_HOME}/git,sharing=locked \
    --mount=type=cache,target=/cargo-target,sharing=locked \
    --mount=type=cache,target=/opt/sccache,sharing=shared \
    cargo chef cook --release --recipe-path recipe.json

# 4-2. Dependency build base for dev/test
FROM chef AS dev-base-build
COPY --from=tools-builder /usr/local/cargo/bin/sccache /usr/local/cargo/bin/sccache
ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache \
    SCCACHE_DIR=/opt/sccache \
    SCCACHE_IDLE_TIMEOUT=600
COPY --from=planner /workspace/recipe.json recipe.json

# Local dev: keep BuildKit cache mounts for fast rebuild
FROM dev-base-build AS dev-base-local
# vendor/ contains path dependencies (e.g. conch-parser) that cargo chef cook
# needs to resolve. Copy it before cooking so the source is available.
COPY vendor/ vendor/
RUN --mount=type=cache,target=${CARGO_HOME}/registry,sharing=locked \
    --mount=type=cache,target=${CARGO_HOME}/git,sharing=locked \
    --mount=type=cache,target=/cargo-target,sharing=locked \
    --mount=type=cache,target=/opt/sccache,sharing=shared \
    cargo chef cook --recipe-path recipe.json --all-targets --all-features
# dev-base-local cooks into a BuildKit cache mount (not the image layer), so the
# named volume at /cargo-target initialises from an empty dir. Make it
# world-writable so the non-root runtime user can write the build target.
RUN mkdir -p /cargo-target && chmod -R a+rwX /cargo-target

# CI: persist deps in image layers (no BuildKit cache mount on /cargo-target so
# artifacts are baked into the image layer and survive as named-volume initial
# content when Docker initialises cargo_target_cache:/cargo-target at first run).
FROM dev-base-build AS dev-base-ci
ENV CARGO_PROFILE_DEV_DEBUG=0 \
    CARGO_PROFILE_TEST_DEBUG=0
COPY vendor/ vendor/
RUN --mount=type=cache,target=/opt/sccache,sharing=shared \
    cargo chef cook --check --recipe-path recipe.json --all-targets --all-features && \
    cargo chef cook --recipe-path recipe.json --all-targets --all-features && \
    chmod -R a+rwX /cargo-target

FROM dev-base-${BUILD_ENV} AS dev-base

# 5. Release builder for runtime
FROM builder-base AS builder
ARG APP_BIN=server
COPY . .
RUN --mount=type=cache,target=${CARGO_HOME}/registry,sharing=locked \
    --mount=type=cache,target=${CARGO_HOME}/git,sharing=locked \
    --mount=type=cache,target=/cargo-target,sharing=locked \
    --mount=type=cache,target=/opt/sccache,sharing=shared \
    cargo build --release -p ${APP_BIN} && \
    cp /cargo-target/release/${APP_BIN} /bin/server

# 6. Dev watcher image for the optional compose.dev overlay
FROM dev-base AS dev
COPY --from=tools-builder /usr/local/cargo/bin/bacon /usr/local/cargo/bin/
WORKDIR /workspace
CMD ["bacon", "run", "--headless"]

# 7. Tools image used by docker compose
FROM dev-base AS tools
COPY --from=tools-builder /usr/local/cargo/bin/cargo-make /usr/local/cargo/bin/
COPY --from=tools-builder /usr/local/cargo/bin/cargo-nextest /usr/local/cargo/bin/
COPY --from=tools-builder /usr/local/cargo/bin/cargo-deny /usr/local/cargo/bin/
COPY --from=tools-builder /usr/local/cargo/bin/cargo-machete /usr/local/cargo/bin/
COPY --from=tools-builder /usr/local/cargo/bin/cargo-llvm-cov /usr/local/cargo/bin/
WORKDIR /workspace
CMD ["bash"]

# 8. Runtime image
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime
WORKDIR /app
COPY --from=builder /bin/server /app/server
EXPOSE 8080
ENTRYPOINT ["/app/server"]
