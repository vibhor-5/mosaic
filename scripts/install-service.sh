#!/usr/bin/env bash
set -e

# Build the release binary
echo "Building mosaic daemon..."
cargo build --release

BIN_PATH="$PWD/target/release/mosaic"
PLIST_SRC="$PWD/scripts/io.mosaic.plist"
PLIST_DEST="$HOME/Library/LaunchAgents/io.mosaic.daemon.plist"

if [ ! -f "$BIN_PATH" ]; then
    echo "Error: Binary not found at $BIN_PATH"
    exit 1
fi

echo "Installing launchd service..."

# Stop existing service if any
launchctl unload "$PLIST_DEST" 2>/dev/null || true

# Copy and configure the plist
mkdir -p "$HOME/Library/LaunchAgents"
sed "s|__BIN_PATH__|$BIN_PATH|g" "$PLIST_SRC" > "$PLIST_DEST"

# Load and start the service
launchctl load "$PLIST_DEST"

echo "Mosaic daemon installed and started via launchd!"
echo "Check logs at /tmp/mosaic.out.log and /tmp/mosaic.err.log"
