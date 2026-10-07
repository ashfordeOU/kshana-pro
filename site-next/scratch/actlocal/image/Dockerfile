# The act job image for the local-runner kit (see site-next/LOCAL-RUNNERS.md).
# rerun.sh builds it twice from this one file, on first use:
#   docker build --platform linux/arm64 -t kshana-act:ubuntu-latest           image/
#   docker build --platform linux/amd64 -t kshana-act:ubuntu-latest-amd64     image/
# It is the medium catthehacker act image (multi-arch) plus the two things GitHub's
# ubuntu-latest runner ships and the kshana workflows use without installing: rustup
# (with the repo's pinned toolchain as default) and rsync. Workflow steps such as
# dtolnay/rust-toolchain@1.93.0 / @1.85.0 still install their own toolchain through
# this rustup, exactly as on GitHub.
FROM catthehacker/ubuntu:act-latest

ARG RUST_TOOLCHAIN=1.93.0

RUN apt-get update \
 && apt-get install -y --no-install-recommends rsync \
 && rm -rf /var/lib/apt/lists/*

# Same layout as GitHub's runner image: RUSTUP_HOME/CARGO_HOME under the home dir, on PATH.
ENV RUSTUP_HOME=/root/.rustup \
    CARGO_HOME=/root/.cargo \
    PATH=/root/.cargo/bin:$PATH

# rustup-init picks the host triple itself, so one file serves arm64 and amd64.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --no-modify-path --profile minimal \
           --default-toolchain "$RUST_TOOLCHAIN" -c rustfmt -c clippy \
 && rustup --version && cargo --version && rustc --version && rsync --version >/dev/null
