#!/bin/bash
# Helper script to generate and add a test case to the CSV file
#
# Usage: ./scripts/add_test_case.sh "MESSAGE TEXT"
#        ./scripts/add_test_case.sh "MESSAGE TEXT" "EXPECTED DECODED"
#
# If EXPECTED_DECODED is not provided, it defaults to MESSAGE TEXT

if [ $# -lt 1 ]; then
    echo "Usage: $0 \"MESSAGE TEXT\" [\"EXPECTED DECODED\"]"
    echo ""
    echo "Examples:"
    echo "  $0 \"CQ DX K1ABC FN42\""
    echo "  $0 \"CQ PJ4/K1ABC\" \"CQ <PJ4/K1ABC>\""
    exit 1
fi

MESSAGE="$1"
EXPECTED_DECODED="${2:-$MESSAGE}"
FT8CODE="/workspaces/RustyFt8/wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/ft8code"
CSV_FILE="/workspaces/RustyFt8/tests/message/encode_decode_cases.csv"

if [ ! -f "$FT8CODE" ]; then
    echo "Error: ft8code not found at $FT8CODE"
    echo "Please build WSJT-X first"
    exit 1
fi

# Run ft8code and extract the 77-bit message
BITS=$("$FT8CODE" "$MESSAGE" 2>/dev/null | grep -A1 "Source-encoded message" | tail -1 | tr -d ' ')

if [ -z "$BITS" ]; then
    echo "Error: Could not encode message with ft8code"
    echo "Output from ft8code:"
    "$FT8CODE" "$MESSAGE"
    exit 1
fi

# Check if bits are exactly 77 characters
if [ ${#BITS} -ne 77 ]; then
    echo "Error: Expected 77 bits, got ${#BITS}"
    echo "Bits: $BITS"
    exit 1
fi

# Append to CSV file
echo "$MESSAGE,$BITS,$EXPECTED_DECODED" >> "$CSV_FILE"

echo "✓ Added test case to $CSV_FILE:"
echo "  Message: $MESSAGE"
echo "  Bits:    $BITS"
echo "  Decoded: $EXPECTED_DECODED"
echo ""
echo "The test will automatically pick up the new case on next run."
