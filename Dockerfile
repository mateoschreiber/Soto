# syntax=docker/dockerfile:1
FROM rust:1.99.1-trixie AS build
ARG DX_VERSION=0.7.10
ARG TARGETARCH
WORKDIR /src
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates unzip \
    && rm -rf /var/lib/apt/lists/*
RUN case "$TARGETARCH" in amd64) DX_ARCH=x86_64;; arm64) DX_ARCH=aarch64;; *) exit 1;; esac \
    && curl -fsSL "https://github.com/DioxusLabs/dioxus/releases/download/v${DX_VERSION}/dx-${DX_ARCH}-unknown-linux-gnu.tar.gz" -o /tmp/dx.tar.gz \
    && curl -fsSL "https://github.com/DioxusLabs/dioxus/releases/download/v${DX_VERSION}/dx-${DX_ARCH}-unknown-linux-gnu.sha256" -o /tmp/dx.sha256 \
    && cd /tmp && printf '%s  dx.tar.gz\n' "$(cut -d ' ' -f 1 dx.sha256)" | sha256sum -c - \
    && tar xzf dx.tar.gz -C /usr/local/bin dx && rm dx.tar.gz dx.sha256
ENV RUSTUP_TOOLCHAIN=1.99.1
RUN rustup target add wasm32-unknown-unknown && rustup component add llvm-tools-preview
COPY Cargo.toml Cargo.lock rust-toolchain.toml Dioxus.toml ./
COPY crates ./crates
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p soto-server \
    && dx build -p soto-web --platform web --profile wasm-release --release --locked \
    && mkdir -p /out/public \
    && cp target/release/soto-server /out/soto-server \
    && cp -r target/dx/soto-web/release/web/public/. /out/public/ \
    && find /out/public -type f \( -name '*.wasm' -o -name '*.js' -o -name '*.css' \) -exec gzip -9 -k {} \;

FROM debian:trixie-slim AS runtime
RUN useradd --system --uid 10001 --no-create-home soto
WORKDIR /app
COPY --from=build /out/ /app/
USER 10001:10001
ENV SOTO_BIND=0.0.0.0:3000 SOTO_STATIC_DIR=/app/public RUST_LOG=soto_server=info,tower_http=warn
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 CMD ["/app/soto-server", "--healthcheck"]
ENTRYPOINT ["/app/soto-server"]
