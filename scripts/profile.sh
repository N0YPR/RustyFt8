#!/bin/bash
# Profile script for FT8 decoder

set -e

echo "=== Profiling FT8 Decoder ==="
echo ""

# Build release binary
echo "Building release binary..."
cargo build --release --bin ft8detect

# Run with single recording and measure time
echo ""
echo "Testing decode performance on 210703_133430.wav..."
time ./target/release/ft8detect tests/test_data/210703_133430.wav

echo ""
echo "=== Profile Complete ==="
