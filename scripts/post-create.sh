#!/bin/bash
# Post-create script for devcontainer
# Merges custom CA certificates with system certificates

set -e

CUSTOM_CA_CERT="/home/vscode/ca-certificates.crt"
SYSTEM_CA_DIR="/usr/local/share/ca-certificates"
SYSTEM_CA_FILE="custom-ca.crt"

# Check if custom CA certificate exists
if [ -f "$CUSTOM_CA_CERT" ]; then
    echo "Found custom CA certificate at $CUSTOM_CA_CERT"

    # Copy the certificate to the system CA directory
    # This requires sudo, so we need to run it as root
    echo "Merging custom CA certificate with system certificates..."
    sudo cp "$CUSTOM_CA_CERT" "$SYSTEM_CA_DIR/$SYSTEM_CA_FILE"

    # Update CA certificates
    sudo update-ca-certificates

    echo "Custom CA certificate successfully merged with system certificates"
else
    echo "No custom CA certificate found at $CUSTOM_CA_CERT - skipping certificate merge"
fi

# Show Rust toolchain info
echo "Rust toolchain information:"
rustup show
cargo --version

echo "Post-create setup complete!"
