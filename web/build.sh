#!/bin/sh
set -eu
cd "$(dirname "$0")"

required_version=0.2.125
if ! command -v wasm-bindgen >/dev/null 2>&1 || [ "$(wasm-bindgen --version)" != "wasm-bindgen $required_version" ]; then
    echo "Install the matching binding generator: cargo install wasm-bindgen-cli --version $required_version --locked" >&2
    exit 1
fi

if ! rustup target list --installed | grep -qx wasm32-unknown-unknown; then
    echo 'Install the browser target: rustup target add wasm32-unknown-unknown' >&2
    exit 1
fi

# An explicit target directory also works when CARGO_TARGET_DIR is set by the caller.
cargo build --manifest-path wasm/Cargo.toml --target-dir wasm/target \
    --target wasm32-unknown-unknown --release --locked
wasm-bindgen wasm/target/wasm32-unknown-unknown/release/mongol_norm_web.wasm \
    --target web --out-dir public/pkg --out-name mongol_norm
echo 'Built web/public — serve this directory with any static HTTP server.'
