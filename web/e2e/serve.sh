#!/bin/sh
# Serveur des tests de bout en bout : base neuve, jeu de démonstration, date figée (ADR-0018).
set -eu
cd "$(dirname "$0")/../.."
DATA=target/e2e-data
TODAY=2026-10-20
rm -rf "$DATA"
cargo build -q -p cashflow-server
./target/debug/cashflow --data-dir "$DATA" demo --today "$TODAY"
exec ./target/debug/cashflow --data-dir "$DATA" serve --addr 127.0.0.1:3100 --web-dir web/dist/cashflow/browser --today "$TODAY"
