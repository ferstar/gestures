#!/bin/bash
set -euo pipefail

# Build script for maximum Linux compatibility using manylinux
# This script runs inside a manylinux Docker container

echo "==> Building gestures for maximum Linux compatibility"
echo "Target: ${TARGET}"

# Show proxy configuration if set
if [ -n "${http_proxy:-}" ] || [ -n "${https_proxy:-}" ]; then
    echo "Proxy configuration:"
    [ -n "${http_proxy:-}" ] && echo "  http_proxy: ${http_proxy}"
    [ -n "${https_proxy:-}" ] && echo "  https_proxy: ${https_proxy}"
fi

# Set up Cargo home
export CARGO_HOME="${CARGO_HOME:-/rust/cargo}"
export RUSTUP_HOME="${RUSTUP_HOME:-/rust/rustup}"

# Install Rust if not present
if ! command -v rustc &> /dev/null; then
    echo "==> Installing Rust toolchain"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
    source "${CARGO_HOME}/env"
else
    echo "==> Using cached Rust toolchain"
    source "${CARGO_HOME}/env"
fi

# Add target if needed
rustup target add "${TARGET}" || echo "Target already added"

# Install system dependencies
echo "==> Installing system dependencies"
yum install -y \
    systemd-devel \
    libinput-devel \
    libxdo-devel \
    libevdev-devel \
    gcc \
    gcc-c++ \
    make \
    pkgconfig

# Build the project
echo "==> Building release binary"
cargo build --release --target "${TARGET}" --verbose

# Strip the binary
echo "==> Stripping binary"
strip "target/${TARGET}/release/gestures"

# Verify glibc compatibility
echo "==> Checking glibc version requirement"
echo "glibc requirement:"
objdump -T "target/${TARGET}/release/gestures" | grep GLIBC | sed 's/.*GLIBC_\([.0-9]*\).*/\1/g' | sort -Vu | tail -1

echo "==> Build complete"
ls -lh "target/${TARGET}/release/gestures"
