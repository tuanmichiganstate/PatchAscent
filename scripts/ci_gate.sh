#!/usr/bin/env bash
set -euo pipefail

patchascent_cargo_bin="${PATCHASCENT_CARGO_BIN:-cargo}"
patchascent_python_bin="${PATCHASCENT_PYTHON_BIN:-python3}"

npm run quality
"${patchascent_python_bin}" scripts/validate_registry.py protocol/parameter_registry.yaml
"${patchascent_cargo_bin}" fmt --all -- --check
"${patchascent_cargo_bin}" clippy --workspace --all-targets -- -D warnings
"${patchascent_cargo_bin}" clippy --workspace --all-targets --all-features -- -D warnings
"${patchascent_cargo_bin}" test --workspace
"${patchascent_cargo_bin}" test --workspace --all-features
