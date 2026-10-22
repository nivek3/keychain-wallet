#!/bin/bash
set -euo pipefail
umask 077
root=$(cd "$(dirname "$0")/.." && pwd)
export CARGO_TARGET_DIR="$root/target"
cargo test --workspace --locked --manifest-path "$root/Cargo.toml"
python3 -m unittest discover -s "$root/tests" -p 'test_*.py'
