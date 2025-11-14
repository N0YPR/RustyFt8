#!/bin/bash
# Helper script to generate test cases from ft8code output
# Usage: ./generate_test_case.sh "MESSAGE TEXT"

set -e

FT8CODE="/workspaces/RustyFt8/wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/ft8code"

if [ $# -eq 0 ]; then
    echo "Usage: $0 \"MESSAGE TEXT\""
    echo ""
    echo "Example:"
    echo "  $0 \"CQ N0YPR DM42\""
    echo ""
    echo "Output will be in the format ready to paste into tests/integration_test.rs"
    exit 1
fi

MESSAGE="$1"

echo "Running ft8code for: $MESSAGE"
echo ""

# Run ft8code and capture output
OUTPUT=$("$FT8CODE" "$MESSAGE" 2>&1)

# Extract the 77-bit source-encoded message
# The output format from ft8code typically shows the bits in a specific line
# This is a placeholder - you'll need to adjust based on actual ft8code output format
BITS=$(echo "$OUTPUT" | grep -oP '\d{77}' | head -1)

if [ -z "$BITS" ]; then
    echo "ERROR: Could not extract 77-bit message from ft8code output"
    echo ""
    echo "Full output:"
    echo "$OUTPUT"
    exit 1
fi

# Generate the test case
echo "#[case::TODO_NAME(Ft8CodeReference {"
echo "    message: \"$MESSAGE\","
echo "    expected_bits: \"$BITS\","
echo "    expected_decoded: None, // TODO: Update if message decodes differently"
echo "})]"
echo ""
echo "Raw ft8code output:"
echo "$OUTPUT"
