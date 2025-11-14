#!/bin/bash
# Wrapper script for running WSJT-X ft8code tool
# 
# Usage: ./ft8code.sh "MESSAGE TEXT"
#
# This script checks if ft8code is available and runs it with the provided arguments.
# If not found, it provides instructions for building WSJT-X.

FT8CODE_PATH="/workspaces/RustyFt8/wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/ft8code"

if [ ! -f "$FT8CODE_PATH" ]; then
    echo "Error: ft8code not found at: $FT8CODE_PATH"
    echo ""
    echo "Please build WSJT-X first. See instructions in:"
    echo "  - WSJTX.MD"
    echo "  - TESTING.md"
    echo ""
    echo "Quick summary:"
    echo "  1. Download WSJT-X source to wsjtx/"
    echo "  2. Apply patches and build"
    echo "  3. ft8code will be available in the build directory"
    exit 1
fi

if [ $# -eq 0 ]; then
    echo "Usage: $0 \"MESSAGE TEXT\""
    echo ""
    echo "Example: $0 \"CQ DX W1AW FN31\""
    exit 1
fi

# Run ft8code with all provided arguments
"$FT8CODE_PATH" "$@"
