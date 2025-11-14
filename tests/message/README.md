# Data-Driven Testing Guide

## Overview

The test cases for `test_encode_decode_roundtrip` are stored in a CSV file rather than hardcoded in the test file. This makes it easy to add, modify, and review test cases without cluttering the code.

## Test Data File

**Location**: `tests/message/encode_decode_cases.csv`

**Format**: CSV with 3 columns (no header)
```
message,expected_bits,expected_decoded
```

- **message**: The FT8 message text to encode
- **expected_bits**: The expected 77-bit output (binary string). Wildcards supported: any non-0/1 character (like `.`, `x`, `-`) means "don't care"
- **expected_decoded**: What the message should decode to (may differ from input for nonstandard callsigns)

**Example**:
```csv
# Standard QSO
CQ DX K1ABC FN42,00000000000000000000000000100000010100100110110011100110100001100111110010001,CQ DX K1ABC FN42

# Nonstandard callsign (decodes with angle brackets)
CQ PJ4/K1ABC,xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx,CQ <PJ4/K1ABC>

# Partial validation with wildcards (only check i3 field, bits 74-76)
KK7JXP N0YPR/R +10,..........................................................................001,KK7JXP N0YPR/R +10
```

## Adding Test Cases

### Method 1: Using the Helper Script (Recommended)

```bash
# Add a test case (decoded text same as input)
./scripts/add_test_case.sh "CQ DX K1ABC FN42"

# Add a test case where decoded text differs (e.g., nonstandard callsigns)
./scripts/add_test_case.sh "CQ PJ4/K1ABC" "CQ <PJ4/K1ABC>"
```

The script will:
1. Run ft8code to get the reference bits
2. Append the test case to the CSV file

The test automatically picks up all cases from the CSV - no manual updates needed!

**Note**: If the test fails with a decoded text mismatch (e.g., signal reports drop leading zeros like "-05" → "-5"), manually edit the CSV file to fix the expected_decoded column.

### Method 2: Manual Addition

1. Generate the bits using ft8code:
   ```bash
   /workspaces/RustyFt8/wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/ft8code "YOUR MESSAGE"
   ```

2. Copy the "Source-encoded message" line (77 bits)

2. Add a line to `tests/message/encode_decode_cases.csv`:
   ```csv
   YOUR MESSAGE,0101010...(77 bits)...,EXPECTED DECODED
   ```

That's it! The test automatically picks up all cases from the CSV.

## Benefits

✅ **Clean Code**: Tests don't get cluttered with data  
✅ **Easy to Add**: Just append to CSV file  
✅ **Easy to Review**: CSV diffs are readable in git  
✅ **Wildcard Support**: Test incremental implementation  
✅ **Automated**: Script generates test cases from ft8code  
✅ **Version Control**: CSV changes show exactly what tests changed  
✅ **Automatic Discovery**: No need to manually update test counts

## How It Works

The test iterates through all cases in the CSV file automatically:
```rust
#[test]
fn test_encode_decode_roundtrip() {
    let cases = test_cases(); // Loads all cases from CSV
    for (idx, test) in cases.iter().enumerate() {
        // Test each case...
    }
}
```

Just add rows to the CSV and the test picks them up on the next run!

## Wildcard Examples

Test only specific fields by using wildcards for bits you haven't implemented yet:

```csv
# Only test i3 field (last 3 bits)
CQ K1ABC FN42,..........................................................................001,CQ K1ABC FN42

# Test callsign and i3, skip grid
CQ K1ABC FN42,0000000000000000000000000010...............x...........................001,CQ K1ABC FN42

# Full validation (no wildcards)
CQ K1ABC FN42,00000000000000000000000000100000010100100110110011100110100001100111110010001,CQ K1ABC FN42
```

## Running Tests

```bash
# Run all roundtrip tests
cargo test test_encode_decode_roundtrip

# Run a specific case (rstest generates one test per case)
cargo test test_encode_decode_roundtrip::case_idx_0

# Run with output
cargo test test_encode_decode_roundtrip -- --nocapture
```
