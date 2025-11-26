# Subtraction Debugging Session - 2025-11-25

## Summary

Fixed critical time offset bug in signal subtraction. Multi-pass decoding now working: **7 → 9 decodes** (29% improvement), though still below WSJT-X's 22 decodes.

## Bug Found and Fixed

### The Problem

Signal subtraction was showing ~0 dB power change because we were looking in the **wrong place** in the audio buffer.

**Root cause**: Time offsets are relative to 0.5s, not 0.0s!

- [fine.rs:152](../src/sync/fine.rs#L152): `time_offset + 0.5` when computing sample indices
- [fine.rs:225](../src/sync/fine.rs#L225): `refined_time = (best_time / rate) - 0.5`
- [subtract.rs:226](../src/subtract.rs#L226): **Used `time_offset` directly** → OFF BY 0.5 SECONDS!

### The Fix

```rust
// BEFORE (subtract.rs:226)
let nstart = (time_offset * SAMPLE_RATE) as i32;

// AFTER
let absolute_time = time_offset + 0.5;
let nstart = (absolute_time * SAMPLE_RATE) as i32;
```

**Impact**: 0.5 seconds = **6000 samples** offset at 12 kHz!

### Verification

**Before fix**:
```
camp_mag=1.775e-3      (amplitude estimate: ~0.002) ← Almost no correlation!
reconstructed_power=3.547e-3
power_before=4.822e1
power_after=4.821e1    → 0.0 dB change
```

**After fix**:
```
camp_mag=7.590e0       (amplitude estimate: ~7.6) ← 1000x better!
reconstructed_power=1.517e1
power_before=5.371e1
power_after=3.217e1    → -2.2 dB reduction ✓
```

## Test Results

### Before Fix
- **Pass 1**: 7 decodes
- **Pass 2**: 0 decodes (stopped)
- **Total**: 7/22 (32%)

### After Fix
- **Pass 1**: 7 decodes
- **Pass 2**: 1 decode (2695.3 Hz)
- **Pass 3**: 1 decode (398.9 Hz)
- **Total**: 9/22 (41%)

### WSJT-X Baseline
- **Total**: 22/22 (100%)

## What's Working

### Multi-Pass Loop ✓
Passes 1, 2, 3 all run and find new signals after subtraction.

### Time Offset Fix ✓
Signals now correlate with synthesized waveforms:
- W1FC @ 2572 Hz: -2.2 dB reduction, camp_mag=7.6
- WM3PEN @ 2157 Hz: -4.9 dB reduction, camp_mag=11.0

### Pass 2/3 Decodes ✓
- **Pass 2**: 398.9 Hz "W0RSJ EA3BMU RR73" (WSJT-X: 400 Hz ✓)
- **Pass 3**: Additional weak signal found

## What's Not Working

### 1. Incomplete Subtraction

Most signals still show ~0 dB power change:
- 2854.9 Hz (XE2X): camp_mag=0.012, -0.0 dB
- 2238.3 Hz (N1API): camp_mag=0.023, -0.0 dB
- 2733.4 Hz (W1DIG): camp_mag=0.055, -0.1 dB
- 589.4 Hz (K1JT): camp_mag=0.015, -0.0 dB
- 642.0 Hz (N1JFU): camp_mag=0.008, -0.0 dB

**Only 2 out of 7 signals** show significant subtraction!

### 2. False Positive in Pass 2

**Pass 2 @ 2695.3 Hz**: "J9BFQ ZM5FEY R QA56"
**WSJT-X @ 2695 Hz**: "K1BZM EA3GP -09"

This is an LDPC false positive - the OSD decoder found a valid codeword but it's the wrong message.

### 3. Still Missing 13 Signals

Missing strong signals that WSJT-X decodes:
- CQ F5RXL @ 1197 Hz (-2 dB)
- N1PJT HB9CQK @ 466 Hz (-2 dB)
- KD2UGC F6GCP @ 472 Hz (-6 dB)
- K1BZM EA3GP @ 2695 Hz (-3 dB) ← Got false positive instead
- Plus 9 more weaker signals

## Root Cause Analysis

### Why Only 2/7 Signals Subtract Well?

Looking at the successful ones:
- W1FC @ 2572 Hz: Strong signal (-8 dB), good sync, clean decode
- WM3PEN @ 2157 Hz: Strong signal (-4 dB), good sync, clean decode

Looking at the failures:
- XE2X @ 2854 Hz: Weak signal (-14 dB), camp_mag=0.012
- N1API @ 2238 Hz: Weak signal (-12 dB), camp_mag=0.023
- W1DIG @ 2733 Hz: Weak signal (-11 dB), camp_mag=0.055

**Pattern**: Weak signals have poor correlation even after time offset fix!

### Possible Causes

1. **Pulse synthesis mismatch**: Our `pulse::generate_complex_waveform()` doesn't perfectly match the transmitted signal
   - Phase continuity issues?
   - GFSK pulse shape differences?
   - Amplitude not normalized correctly?

2. **Frequency offset**: Fine sync finds frequency ±0.5 Hz, but actual signal might be off by more
   - Small frequency error accumulates over 12.64 seconds
   - Phase drift makes correlation worse

3. **Tone errors in weak signals**: LDPC corrects some bit errors, but re-encoded tones might still be wrong
   - If decoded message is slightly wrong, tones are wrong
   - Wrong tones → synthesized signal doesn't match → poor subtraction

4. **SNR threshold for subtraction**: Weak signals (-11 to -14 dB) might be below noise floor
   - Hard to estimate amplitude when signal buried in noise
   - camp_mag stays small even with correct alignment

## Comparison: Working vs Non-Working Subtraction

### W1FC @ 2572 Hz (WORKING)
```
SNR: -8 dB (relatively strong)
camp_mag: 7.590
reconstructed_power: 15.17
power_before: 53.71
power_after: 32.17
Reduction: -2.2 dB ✓
```

### XE2X @ 2854 Hz (NOT WORKING)
```
SNR: -14 dB (weak)
camp_mag: 0.012 (500x smaller!)
reconstructed_power: 0.023
power_before: 32.35
power_after: 32.32
Reduction: -0.0 dB ✗
```

**Hypothesis**: Subtraction only works for signals above ~-10 dB SNR.

## Next Steps

### Priority 1: Verify Pulse Synthesis

Compare our synthesized signals with actual FT8 transmissions:
1. Decode a clean signal with known message
2. Generate same message with our pulse synthesis
3. Cross-correlate to check phase/amplitude match
4. Adjust pulse::generate_complex_waveform() if needed

### Priority 2: Improve Weak Signal Handling

For signals below -10 dB SNR:
1. Don't attempt subtraction (correlation too poor)
2. Or use multi-symbol averaging to improve SNR estimate
3. Or use decoded message confidence (LDPC iterations) to filter

### Priority 3: Filter False Positives

The 2695 Hz false positive suggests:
1. Tighten SNR threshold for Pass 2+ candidates
2. Require higher sync quality after subtraction
3. Check if LDPC used many OSD iterations (sign of low confidence)
4. Validate decoded message format more strictly

### Priority 4: Fix nsym=2/3

Re-enable multi-symbol coherent combining with phase tracking:
- Would improve weak signal detection
- WSJT-X uses this heavily
- Needs per-symbol phase estimation from Costas arrays

## Performance Metrics

| Metric | Before | After | Target (WSJT-X) |
|--------|--------|-------|-----------------|
| Total decodes | 7/22 (32%) | 9/22 (41%) | 22/22 (100%) |
| Pass 1 | 7 | 7 | ~12-15 |
| Pass 2 | 0 | 1 | ~5-7 |
| Pass 3 | 0 | 1 | ~2-3 |
| Effective subtractions | 0/7 (0%) | 2/7 (29%) | ~6/7 (86%) |
| False positives | 0 | 1 | 0 |

## Conclusion

Fixing the time offset bug enabled multi-pass decoding to work, improving from 7 to 9 decodes. However:

1. **Subtraction effectiveness is limited**: Only 2/7 signals show significant power reduction
2. **Weak signals don't subtract**: Signals below -10 dB SNR have poor correlation
3. **False positives appear**: Need better filtering for low-confidence decodes

The time offset fix was critical, but we need better pulse synthesis or handling of weak signals to match WSJT-X's 22/22 performance.

## Files Modified

- [src/subtract.rs](../src/subtract.rs): Fixed time offset bug (lines 226-229, 177-178)
  - Added `+ 0.5` to convert relative time to absolute time
  - Added debug output for camp_mag and reconstructed_power
  - Both in internal function and time refinement loop

## Key Code Changes

```rust
// Line 226-229: Fixed absolute time calculation
let absolute_time = time_offset + 0.5;  // ← Added 0.5
let nstart = (absolute_time * SAMPLE_RATE) as i32;

// Line 177-178: Fixed power measurement
let absolute_test_time = test_time + 0.5;  // ← Added 0.5
let nstart = (absolute_test_time * SAMPLE_RATE) as i32;
```
