#!/usr/bin/env bash

# Mosaic Scripting Addition Injector
# 
# This script injects the Mosaic payload into the macOS Dock.
# IMPORTANT: This requires System Integrity Protection (SIP) to be partially disabled
# (specifically `csrutil enable --without fs --without debug`), similar to yabai.

echo "[*] Locating Dock.app..."
DOCK_PID=$(pgrep Dock)

if [ -z "$DOCK_PID" ]; then
    echo "[-] Could not find Dock.app PID"
    exit 1
fi

PAYLOAD_PATH="$(pwd)/sa/payload.dylib"

if [ ! -f "$PAYLOAD_PATH" ]; then
    echo "[-] Payload not found at $PAYLOAD_PATH"
    echo "[-] Please run 'make -C sa' first"
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
