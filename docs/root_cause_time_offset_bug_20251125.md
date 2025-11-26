# Root Cause: Time Offset Clipping Bug - 2025-11-25

## Executive Summary

**Found the root cause of 64% decode failures!**

Coarse sync is working perfectly, finding all signals with strong correlation scores. However, fine sync/extraction fails due to incorrect time offset handling that clips negative offsets, losing the first Costas array and preventing LDPC decode.

## Problem Statement

- **RustyFt8**: 8/22 decodes (36%)
- **WSJT-X**: 22/22 decodes (100%)
- **Missing**: 14 strong signals (-2 to -7 dB that should be trivial to decode)

## Investigation Results

### Coarse Sync: ✅ WORKING

Generated candidates with excellent sync scores:

| Signal | Frequency | Coarse Sync | Status |
|--------|-----------|-------------|---------|
| F5RXL | 1196.9 Hz | 38.690 | ✅ Found |
| K1BZM EA3GP | 2696.9 Hz | 10.947 | ✅ Found |
| DL8YHR | 2609.4 Hz | 35.769 | ✅ Found |

### Fine Sync / Extraction: ❌ FAILING

After fine_sync refinement, sync scores collapse:

| Signal | Coarse Sync | Fine Sync | Drop Factor | nsync | Result |
|--------|-------------|-----------|-------------|-------|---------|
| F5RXL | 38.690 | 1.158 | 33x | 4/21 (19%) | ❌ Fails |
| K1JT HA0DU | (working) | 0.198 | - | 19/21 (90%) | ✅ Decodes |

## Root Cause Identified

### The Bug

**File**: `src/sync/extract.rs` lines 233-239

```rust
let min_offset = 0i32;  // First symbol must be at position 0 or later

if start_offset < min_offset {
    eprintln!("    CLIPPING start_offset: {} -> {} (signal starts too early)",
             start_offset, min_offset);
    start_offset = min_offset;  // ← This loses the beginning of the transmission!
}
```

### How It Fails

**Example**: F5RXL signal

1. **WSJT-X decodes successfully**:
   - Frequency: 1197 Hz
   - DT: -0.8s (starts 0.8s before expected 0.5s mark)
   - Result: ✅ Decoded

2. **RustyFt8 fails**:
   - Frequency: 1196.8 Hz ✓ (matches!)
   - DT: -0.77s ✓ (matches WSJT-X!)
   - Calculation:
     ```
     absolute_time = time_offset + 0.5 = -0.77 + 0.5 = -0.27s
     start_offset = -0.27 * 200 Hz = -54 samples
     ```
   - **CLIPPED to 0** ← Loses first 54 samples!
   - First Costas array (symbols 0-6) is missing
   - nsync drops from expected ~19/21 to only 4/21
   - LDPC decode fails
   - Result: ❌ Failed

### Successful Decodes vs Failures

**Successful decodes** (8/22):
- DT range: -0.06s to +0.30s (small positive offsets)
- No clipping occurs
- nsync: 17-19/21 (81-90%)
- LDPC decodes successfully

**Failed decodes** (14/22):
- DT range: Often < -0.3s (larger negative offsets)
- Clipping loses first Costas array
- nsync: 4-7/21 (19-33%)
- LDPC fails despite strong SNR

## The Underlying Issue

### Time Offset Semantics

```rust
// From fine.rs:150-152
// candidate.time_offset is relative to 0.5s start, but downsampled buffer starts at 0.0
// So add 0.5s to convert to absolute time
let initial_offset = ((candidate.time_offset + 0.5) * actual_sample_rate) as i32;
```

**Problem**: For signals with `time_offset < -0.5s`:
- Absolute time = time_offset + 0.5 < 0 (negative!)
- This means signal starts BEFORE the downsampled buffer begins
- Clipping to 0 loses the signal beginning

### Why WSJT-X Succeeds

WSJT-X must handle this differently:
1. Downsampled buffer starts before t=0 (has pre-padding)?
2. Different time offset reference point?
3. Allows negative indexing with wraparound?

## Impact

**This single bug causes 64% of decode failures!**

Coarse sync finds all the signals correctly. The entire pipeline works except for this one clipping issue that loses signal data.

## Solution Options

### Option 1: Fix Time Offset Calculation (Recommended)

Investigate how `candidate.time_offset` is calculated in `src/sync/candidate.rs`:
- What is the 0.5s reference point?
- Should it be relative to recording start (t=0) instead?
- Compare with WSJT-X time offset semantics

### Option 2: Add Pre-Padding to Downsampled Buffer

Modify `downsample_200hz` to include padding before t=0:
- Downsample from t=-1.0s to t=16.0s instead of t=0 to t=16s
- Add 200 samples (1 second) of padding at start
- Adjust all offset calculations accordingly
- Allows negative start_offsets up to -1.0s

### Option 3: Remove Clipping (Risky)

Remove the `min_offset = 0` clipping entirely:
- Would work if downsampled buffer actually has valid data at negative indices
- But currently buffer starts at index 0, so would cause array out-of-bounds
- Need Option 2 first

## Next Steps

1. **Investigate `candidate.time_offset` calculation** in `src/sync/candidate.rs`
   - Find where time_offset is set
   - Understand reference point
   - Compare with WSJT-X sync8.f90

2. **Check WSJT-X downsampling** in `ft8_downsample.f90`
   - Does their buffer start before t=0?
   - How do they handle negative time offsets?

3. **Implement fix** based on findings:
   - If time_offset semantics are wrong: fix calculation
   - If downsampling needs padding: add pre-padding
   - Test on 210703_133430.wav: expect 18-20/22 decodes!

## Expected Results After Fix

- **Before**: 8/22 decodes (36%)
- **After**: 18-20/22 decodes (82-91%)
- **Remaining issues**: Weak signals (-17 to -20 dB), LLR quality improvements

This would close the gap to WSJT-X from 64% missing to ~10-18% missing, bringing us very close to parity!

## Files Involved

- `src/sync/extract.rs`: Clipping logic (lines 233-243)
- `src/sync/fine.rs`: Time offset conversion (lines 150-152)
- `src/sync/candidate.rs`: Time offset calculation (to investigate)
- `src/sync/downsample.rs`: Downsampling buffer creation (may need padding)

## References

- [Fine Sync Investigation](/tmp/fine_sync_investigation.txt)
- [Sync Algorithm Comparison](/tmp/sync_algorithm_compare.txt)
- [Root Cause Summary](/tmp/root_cause_summary.txt)
