#!/usr/bin/env bash
# scripts/run-api.sh
set -euo pipefail

cargo build -p litm-api
sudo setcap cap_net_raw,cap_net_admin=eip target/debug/litm-api
exec target/debug/litm-api "$@"
