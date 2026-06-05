#!/usr/bin/env bash
# install.sh — Build and install LucidPM binaries
#
# Usage:
#   ./install.sh                      # installs to ~/.local/bin
#   INSTALL_DIR=/usr/local/bin ./install.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

MODULES=(
  item_links
  item_status
  journal
  logseq_export
  logseq_sync
  multi_project
  ontology_suggest
  pm_structuring
  priority_view
  project_schema
  project_state
  report_export
  task_model
)

echo "LucidPM installer"
echo "Install directory: $INSTALL_DIR"
echo ""

mkdir -p "$INSTALL_DIR"

for module in "${MODULES[@]}"; do
  echo "Building $module..."
  cargo build --release --manifest-path "$SCRIPT_DIR/modules/$module/Cargo.toml" --quiet
  cp "$SCRIPT_DIR/modules/$module/target/release/$module" "$INSTALL_DIR/$module"
  echo "  -> $INSTALL_DIR/$module"
done

echo ""
echo "Installing lucid dispatcher..."
cp "$SCRIPT_DIR/bin/lucid" "$INSTALL_DIR/lucid"
chmod +x "$INSTALL_DIR/lucid"
echo "  -> $INSTALL_DIR/lucid"

echo ""
echo "Installing default vocabulary schema..."
mkdir -p "$HOME/.lucidpm"
cp "$SCRIPT_DIR/config/default-schema.yaml" "$HOME/.lucidpm/default-schema.yaml"
echo "  -> $HOME/.lucidpm/default-schema.yaml"

echo ""
echo "Done. Installed $(( ${#MODULES[@]} + 2 )) files to $INSTALL_DIR"
echo ""

if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
  echo "NOTE: $INSTALL_DIR is not in your PATH."
  echo "Add this to your shell profile (~/.bashrc or ~/.zshrc):"
  echo ""
  echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
  echo ""
fi

echo "Try it: lucid help"
