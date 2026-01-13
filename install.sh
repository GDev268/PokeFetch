#!/bin/sh
set -e

BIN_NAME="pokefetch"        # your Rust binary name
INSTALL_DIR="$HOME/.local/bin"  # target install directory

echo "==> Installing $BIN_NAME to $INSTALL_DIR"

# Ensure install directory exists
mkdir -p "$INSTALL_DIR"

# Copy binary
cp "target/release/$BIN_NAME" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/$BIN_NAME"

echo "Installation complete."

# Optional PATH warning
if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
    echo
    echo "WARNING: $INSTALL_DIR is not in your PATH."
    echo "Add this line to your shell config (~/.zshrc or ~/.bashrc):"
    echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
fi
