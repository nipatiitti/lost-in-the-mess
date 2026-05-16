#!/usr/bin/env bash
# scripts/run-ops.sh
set -euo pipefail
cargo build --bin ops
exec target/debug/ops "$@"