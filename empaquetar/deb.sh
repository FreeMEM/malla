#!/bin/sh
# Crea el .deb para Ubuntu 24.04 o posterior dentro de un contenedor (podman o docker).
# Resultado en dist/.
set -e
cd "$(dirname "$0")/.."
mkdir -p dist
# podman (sin root) si está; si no, docker.
if command -v podman >/dev/null; then ENGINE=podman; OWNER=""; else ENGINE=docker; OWNER="$(id -u):$(id -g)"; fi
$ENGINE run --rm -v "$PWD":/src:Z -w /src docker.io/library/ubuntu:24.04 bash -euc '
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq curl ca-certificates build-essential pkg-config dpkg-dev \
        libgtk-4-dev libadwaita-1-dev libshumate-dev libdbus-1-dev >/dev/null
    curl -sSf https://sh.rustup.rs | sh -s -- -y -q --profile minimal
    . "$HOME/.cargo/env"
    cargo install -q cargo-deb --locked
    export CARGO_TARGET_DIR=/src/target/ubuntu
    cargo deb --output /src/dist/
    if [ -n "'"$OWNER"'" ]; then chown -R "'"$OWNER"'" /src/target/ubuntu /src/dist; fi
'
ls -l dist/*.deb
