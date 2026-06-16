#!/usr/bin/env bash

# Mosaic Scripting Addition Injector
# 
# This script injects the Mosaic payload into the macOS Dock.
# IMPORTANT: This requires System Integrity Protection (SIP) to be partially disabled
# (specifically `csrutil enable --without fs --without debug`), similar to yabai.

echo "[*] Locating Dock.app..."
DOCK_PID=$(pgrep -x Dock)

if [ -z "$DOCK_PID" ]; then
    echo "[-] Could not find Dock.app PID"
    exit 1
fi

# Check common paths for the payload
if [ -f "$(pwd)/sa/payload.dylib" ]; then
    PAYLOAD_PATH="$(pwd)/sa/payload.dylib"
elif [ -f "/opt/homebrew/lib/payload.dylib" ]; then
    PAYLOAD_PATH="/opt/homebrew/lib/payload.dylib"
elif [ -f "/usr/local/lib/payload.dylib" ]; then
    PAYLOAD_PATH="/usr/local/lib/payload.dylib"
else
    echo "[-] Payload not found in standard installation directories."
    echo "[-] If compiling from source, please run 'make -C sa' first."
    exit 1
fi

echo "[*] Found Dock.app (PID: $DOCK_PID)"
echo "[*] Attempting to inject $PAYLOAD_PATH..."

# Use LLDB to force the Dock to dlopen our payload
# This requires debugging entitlements / SIP disabled
sudo lldb -p $DOCK_PID -o "expr (void)dlopen(\"$PAYLOAD_PATH\", 2)" -o "detach" -o "quit"

if [ $? -eq 0 ]; then
    echo "[+] Injection command sent successfully."
    echo "[+] Mosaic now has private SkyLight entitlements."
else
    echo "[-] Injection failed. Is SIP disabled?"
fi
