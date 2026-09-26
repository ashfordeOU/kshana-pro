# Kshana Pro — container image for the hostable service (PR22).
#
# Pro depends on the open core by PATH (`../kshana`), so the build context must be
# the PARENT directory that contains BOTH `kshana/` (open) and `kshana-pro/` (this
# overlay). Build from the parent:
#
#   docker build -f kshana-pro/Dockerfile -t kshana-pro:latest ..
#
# (or use the compose files in deploy/, which set the context to `..`).

# ---- build stage ----
FROM rust:1.93-slim AS build
WORKDIR /src
# Copy both crates so the path dependency resolves.
COPY kshana ./kshana
COPY kshana-pro ./kshana-pro
WORKDIR /src/kshana-pro
# The licence PUBLIC key is compiled into the binary (64 hex characters, from
# `kshana-pro license keygen`). Without it the release binary verifies no token and
# serves every Pro kind at Evaluation tier. Never pass the signing key here.
ARG KSHANA_PRO_LICENSE_PUBKEY=""
ENV KSHANA_PRO_LICENSE_PUBKEY=${KSHANA_PRO_LICENSE_PUBKEY}
RUN cargo build --release --locked

# ---- runtime stage (minimal, non-root) ----
FROM debian:bookworm-slim AS runtime
LABEL org.opencontainers.image.title="Kshana Pro" \
      org.opencontainers.image.vendor="Ashforde OÜ" \
      org.opencontainers.image.description="Proprietary overlay on the open Kshana PNT engine"
# Non-root runtime user.
RUN useradd --system --uid 10001 --no-create-home kshana
COPY --from=build /src/kshana-pro/target/release/kshana-pro /usr/local/bin/kshana-pro
USER kshana
EXPOSE 8088
# Liveness probe actually exercises the HTTP listener's /health route (not just the
# CLI), so a broken/hung server is reported unhealthy.
HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
  CMD ["kshana-pro", "healthcheck", "--addr", "127.0.0.1:8088"]
ENTRYPOINT ["kshana-pro"]
# Fail-closed by default: serves the open engine + free seam-check at Evaluation
# tier. Provide a license to unlock Pro features:
#   command: ["serve","--addr","0.0.0.0:8088","--license","<token>"]
CMD ["serve", "--addr", "0.0.0.0:8088"]
