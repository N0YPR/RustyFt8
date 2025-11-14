# Testing Strategy for RustyFt8

## Overview

This project validates message encoding by comparing full 77-bit message outputs against WSJT-X's `ft8code` reference implementation. This approach allows us to:

1. Split encoding logic into separate modules (callsign, grid, etc.) for clarity
2. Test at the integration level rather than unit-testing each component
3. Have a single source of truth: WSJT-X ft8code output

## Testing Architecture

### Integration Tests (`tests/integration_test.rs`)

The primary test suite compares complete message encoding:

```rust
#[rstest]
#[case::cq_standard(Ft8CodeReference {
    message: "CQ N0YPR DM42",
    expected_bits: "00000000000000000000000000100000010100100110110011100110100001100111110010001",
    expected_decoded: None,
})]
fn test_message_encoding_against_ft8code(#[case] test: Ft8CodeReference) {
    let msg = Message77::from_str(test.message).unwrap();
    let our_bits = bits_to_string(msg.message_bits());
    assert_eq!(our_bits, test.expected_bits);
}
```

### Why This Approach?

**Benefits:**
- ✅ Tests the complete encoding pipeline end-to-end
- ✅ Validates against authoritative reference (WSJT-X)
- ✅ Allows internal refactoring without breaking tests
- ✅ Easy to add new test cases for edge cases
- ✅ Clear pass/fail: either matches ft8code or doesn't

**What we DON'T test:**
- ❌ Individual component functions (callsign encoding, grid encoding, etc.)
- ❌ Internal implementation details
- ❌ Intermediate bit patterns

**Why not unit tests?**
- Components like callsign encoding are complex but their correctness only matters in context
- If the final 77-bit output matches ft8code, all components worked correctly
- Unit tests would require duplicating ft8code's logic, creating maintenance burden

## Adding New Test Cases

### Method 1: Using the Helper Script

```bash
./scripts/generate_test_case.sh "CQ PJ4/K1ABC FN42"
```

This outputs code ready to paste into `tests/integration_test.rs`.

### Method 2: Manual Process

1. **Run ft8code:**
   ```bash
   /workspaces/RustyFt8/wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/ft8code "YOUR MESSAGE"
   ```

2. **Extract the 77-bit source message** from the output

3. **Add test case:**
   ```rust
   #[case::descriptive_name(Ft8CodeReference {
       message: "YOUR MESSAGE",
       expected_bits: "...", // 77-bit string from ft8code
       expected_decoded: None, // or Some("DIFFERENT TEXT") if it decodes differently
   })]
   ```

### Test Case Categories to Cover

Organize test cases by message type and encoding scenarios:

#### Type 1 Messages (i3=1, Standard)
- ✅ `CQ N0YPR DM42` - basic CQ
- ⬜ `N0YPR W1ABC R-10` - signal report
- ⬜ `W1ABC N0YPR RR73` - acknowledgment
- ⬜ `CQ DX K1ABC FN42` - CQ DX

#### Type 4 Messages (i3=4, One Nonstandard Callsign)
- ⬜ `CQ PJ4/K1ABC FN42` - prefix
- ⬜ `CQ K1ABC/P FN42` - suffix
- ⬜ `<...>` callsign hashes

#### Type 2 Messages (i3=2, EU VHF Contest)
- ⬜ Contest exchanges

#### Type 3 Messages (i3=3, RTTY Roundup)
- ⬜ RTTY exchanges

#### Free Text (i3=0, n3=0)
- ⬜ Arbitrary text messages

#### Edge Cases
- ⬜ Maximum length callsigns
- ⬜ Minimum length callsigns
- ⬜ Special prefixes/suffixes
- ⬜ Grid squares at boundaries
- ⬜ All possible signal reports

## Running Tests

```bash
# Run all tests
cargo test

# Run only integration tests
cargo test --test integration_test

# Run specific test case
cargo test --test integration_test -- cq_standard

# Run with output
cargo test --test integration_test -- --nocapture
```

## Development Workflow

1. **Implement encoding logic** in separate modules:
   - `src/message/callsign.rs` - Callsign encoding/decoding
   - `src/message/grid.rs` - Grid square encoding
   - `src/message/encoder.rs` - Message type dispatch and bit packing
   - etc.

2. **Add integration test** with ft8code reference output

3. **Run test** - it will fail initially

4. **Debug** using the compare_ft8code example:
   ```bash
   cargo run --example compare_ft8code
   ```

5. **Iterate** until test passes

6. **Refactor** internal code freely - as long as integration tests pass, you're good!

## Module-Specific Testing (Optional)

You CAN add unit tests within modules for:
- **Helper functions** with simple contracts (e.g., bit manipulation utilities)
- **Internal validation** that catches bugs early
- **Documentation** via test examples

But these are supplementary. The integration tests are the authoritative validation.

Example:
```rust
// In src/message/grid.rs
#[cfg(test)]
mod tests {
    // Optional: test known grid encoding edge cases
    #[test]
    fn test_grid_boundaries() {
        // Only if this helps development
    }
}
```

## Tools

- **`ft8code`** - WSJT-X reference encoder at:
  `/workspaces/RustyFt8/wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/ft8code`

- **`scripts/generate_test_case.sh`** - Helper to create test cases

- **`examples/compare_ft8code.rs`** - Compare specific messages

## Future: Decoder Testing

When implementing the decoder (77 bits → text), use the same pattern:

```rust
#[rstest]
#[case::cq_standard("00000...10001", "CQ N0YPR DM42")]
fn test_message_decoding(#[case] bits: &str, #[case] expected_text: &str) {
    let msg = Message77::from_bits(string_to_bits(bits)).unwrap();
    assert_eq!(msg.decoded_text, expected_text);
}
```

Same philosophy: end-to-end validation against known-good data.
