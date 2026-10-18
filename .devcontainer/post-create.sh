#!/usr/bin/env bash
set -euo pipefail

# Volumes werden als root angelegt – dem vscode-User übergeben
sudo chown -R vscode:vscode /workspaces/.cargo-target /usr/local/cargo/registry /home/vscode/.claude

git config --global --add safe.directory /workspaces/jumpnrun

rustup component add clippy rustfmt
rustup target add wasm32-unknown-unknown

echo
echo "Rust:   $(rustc --version)"
echo "Cargo:  $(cargo --version)"
echo "Claude: $(claude --version 2>/dev/null || echo 'nicht gefunden')"
echo
echo "Spiel-Desktop: http://localhost:6080 (noVNC, kein Passwort)"
