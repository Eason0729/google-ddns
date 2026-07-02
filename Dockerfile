# Multi-arch build: cross-compile on the BUILDPLATFORM host via cargo-zigbuild,
# then manifest both amd64 and arm64 images from a single scratch final stage.
# Mirrors the llumen approach (see github.com/pinkfuwa/llumen).
#
# We use ureq's `native-tls-vendored` feature (bundled OpenSSL) instead of
# rustls/ring, because `ring` does not cross-compile cleanly to aarch64-musl.

FROM --platform=$BUILDPLATFORM rust:1.90-slim-trixie AS builder

RUN apt update -y \
    && apt install -y musl-tools pkg-config make perl curl xz-utils

COPY package/install-zig.sh /usr/local/bin/install-zig.sh
RUN chmod +x /usr/local/bin/install-zig.sh && install-zig.sh
ENV PATH="/opt/zig:$PATH"

RUN --mount=type=cache,target=/root/.cargo/registry/index \
    --mount=type=cache,target=/root/.cargo/registry/cache \
    --mount=type=cache,target=/root/.cargo/git/db \
    cargo install --locked cargo-zigbuild \
    && rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN --mount=type=cache,id=target,target=/build/target \
    cargo zigbuild -r \
        --target x86_64-unknown-linux-musl \
        --target aarch64-unknown-linux-musl \
    && mkdir -p /out/linux \
    && cp target/aarch64-unknown-linux-musl/release/google-ddns /out/linux/arm64 \
    && cp target/x86_64-unknown-linux-musl/release/google-ddns  /out/linux/amd64

# native-tls-vendored embeds the OpenSSL CA bundle, so no CA files needed
# in the final image.
FROM scratch

ARG TARGETPLATFORM
LABEL org.opencontainers.image.title="google-ddns"
LABEL org.opencontainers.image.description="Minimal Google Cloud DNS dynamic updater"
LABEL org.opencontainers.image.source="https://github.com/Eason0729/google-ddns"
LABEL org.opencontainers.image.licenses="MIT"

# Rootless: run as an unprivileged, non-root UID/GID (numeric, no passwd needed in scratch).
USER 65532:65532

WORKDIR /config
VOLUME ["/config"]

COPY --from=builder /out/${TARGETPLATFORM} /google-ddns

ENV RUST_LOG=info
ENV CONFIG_FILE=/config/config.json

CMD ["/google-ddns"]