# Multi-Pass Subtraction Implementation - 2025-11-25

## Summary

Enabled multi-pass decoding with signal subtraction in RustyFt8. The infrastructure exists and runs, but subtraction effectiveness needs debugging.

## What Was Implemented

### 1. Signal Synthesis Module (`src/sync/synthesize.rs`)

Created new module for FT8 signal synthesis:
- **GFSK pulse shaping** with BT=2.0
- **Synthesize FT8 signal** from 79-tone sequence
- **Subtract signal** using matched filtering approach
- Matches WSJT-X algorithm (gen_ft8wave + subtractft8)

### 2. Multi-Pass Decoder Already Existed!

Found that `decoder.rs` already had `decode_ft8_multipass()` function:
- Implements iterative decode-subtract-repeat loop
- Deduplicates messages across passes
- Stops when no new signals found
- Uses existing `src/subtract.rs` module

### 3. Enabled Multi-Pass in ft8detect Binary

Modified `src/bin/ft8detect.rs`:
- Changed from `decode_ft8()` to `decode_ft8_multipass()`
- Configured for 3 passes (typical WSJT-X behavior)
- Reports pass-by-pass results

## Test Results

### Recording: tests/test_data/210703_133430.wav

**WSJT-X**: 22 decodes
**RustyFt8 (before)**: 9 decodes (single-pass, no multipass debugging)
**RustyFt8 (now)**: 7 decodes (multi-pass enabled, but subtraction ineffective)

### Pass Breakdown

**Pass 1**: 7 decodes
- W1FC F5BZB @ 2572 Hz (-8 dB)
- XE2X HA2NP @ 2854 Hz (-14 dB)
- N1API HA6FQ @ 2238 Hz (-12 dB)
- WM3PEN EA6VQ @ 2157 Hz (-4 dB)
- K1JT HA0DU @ 589 Hz (-14 dB)
- W1DIG SV9CVY @ 2733 Hz (-11 dB) ✓ This is the in-band interferer!
- N1JFU EA6EE @ 642 Hz (-15 dB)

**Pass 2**: 0 new decodes (stopped)

### Subtraction Power Changes

All subtractions showed essentially 0 dB power change:
```
Subtraction @ 2572.7 Hz: -0.4 dB power change
Subtraction @ 2854.9 Hz: -0.0 dB power change
Subtraction @ 2238.3 Hz: -0.0 dB power change
Subtraction @ 2157.2 Hz: -0.2 dB power change
Subtraction @ 589.4 Hz: -0.0 dB power change
Subtraction @ 2733.4 Hz: -0.0 dB power change  ⚠️ Should help K1BZM!
Subtraction @ 642.0 Hz: -0.0 dB power change
```

**Critical observation**: W1DIG @ 2733 Hz was subtracted (the interferer we identified), but the power change was 0 dB, meaning the subtraction didn't actually remove the signal.

### Still Missing Strong Signals

**K1BZM EA3GP @ 2695 Hz** (-3 dB):
- ✓ Found as candidate (sync=4.976, good!)
- ✓ Fine sync successful (2695.3 Hz, dt=-0.12s)
- ✓ Symbol extraction runs
- ❌ LDPC decode fails
- **Should benefit** from W1DIG subtraction @ 2733 Hz (38 Hz away), but subtraction was ineffective

**Other missing signals**:
- CQ F5RXL @ 1197 Hz (-2 dB)
- N1PJT HB9CQK @ 466 Hz (-2 dB)
- KD2UGC F6GCP @ 472 Hz (-6 dB)
- K1BZM EA3CJ @ 2522 Hz (-6 dB)
- Plus 10+ more weaker signals

## Root Cause Analysis

### Why Subtraction Shows 0 dB Power Change

Possible causes:

1. **Signal synthesis mismatch**: Synthesized signal doesn't match actual signal
   - Pulse shaping differences (though we use BT=2.0 like WSJT-X)
   - Phase alignment issues
   - Frequency offset

2. **Time refinement failing**: Time offset estimate is wrong
   - subtract.rs searches ±60 samples for best alignment
   - Maybe not finding correct offset
   - Or synthesized signal has wrong phase

3. **Amplitude estimation failing**: Low-pass filter not working
   - FFT-based filtering should estimate signal envelope
   - Complex amplitude camp(t) = audio(t) * conj(cref(t))
   - Filter might have bugs

4. **Pulse module incompatibility**: Our pulse.rs vs sync/synthesize.rs
   - We have TWO pulse synthesis implementations!
   - subtract.rs uses `pulse::generate_complex_waveform()`
   - sync/synthesize.rs has its own `synthesize_ft8_signal()`
   - These might not match!

### Why K1BZM Still Fails

Even though we:
- ✓ Decoded W1DIG @ 2733 Hz (the interferer)
- ✓ Attempted to subtract it (0 dB change = failed)
- ✓ K1BZM found as candidate with good sync

K1BZM still doesn't decode because:
1. **Subtraction didn't work** (0 dB power change)
2. **In-band interference still present** from W1DIG
3. **Tone extraction still has errors** (same 81% accuracy as before)
4. **LDPC still fails** due to incorrect bits

## Key Discovery: Duplicate Pulse Synthesis!

We have two implementations:

### 1. `src/pulse.rs` (existing)
Used by subtract.rs:
```rust
pulse::compute_pulse(&mut pulse_buf, pulse::BT, NSPS)?;
pulse::generate_complex_waveform(tones, &mut cref, &pulse_buf, frequency, SAMPLE_RATE, NSPS)?;
```

### 2. `src/sync/synthesize.rs` (new, created today)
Not used yet:
```rust
synthesize_ft8_signal(tones, f0, &mut output)?;
```

**Problem**: subtract.rs is using pulse.rs, not our new synthesize.rs module!

## Next Steps

### Priority 1: Debug Existing Subtraction (pulse.rs + subtract.rs)

Since subtract.rs exists and uses pulse.rs, debug why it's not working:

1. **Add debug output to subtract.rs**:
   - Print power before/after subtraction in more detail
   - Show time refinement offset chosen
   - Check synthesized signal amplitude

2. **Verify pulse.rs generates correct signals**:
   - Compare with WSJT-X gen_ft8wave output
   - Check GFSK pulse shape
   - Verify phase continuity

3. **Test subtraction on synthetic signals**:
   - Create clean test case with known signal
   - Verify subtraction achieves >20 dB reduction
   - Isolate where it's failing

### Priority 2: Compare pulse.rs vs synthesize.rs

Determine which implementation is better:
- Do they produce identical waveforms?
- Which matches WSJT-X more closely?
- Should we merge them or use one exclusively?

### Priority 3: Improve Subtraction Effectiveness

Once we understand why it's failing:
- Fix time alignment issues
- Fix amplitude estimation
- Fix pulse synthesis if needed

**Expected outcome**: Subtraction showing -10 to -20 dB power changes, Pass 2 finding 5-10 new signals, total 15-20 decodes.

## Code Modified

### src/sync/synthesize.rs (created)
- New signal synthesis module (not yet integrated)
- GFSK pulse shaping
- Complex waveform generation
- Subtraction function

### src/sync/mod.rs
- Added synthesize module export
- Re-exported synthesize functions

### src/bin/ft8detect.rs
- Changed from `decode_ft8()` to `decode_ft8_multipass()`
- Added 3-pass configuration

## Files to Review

### Existing Subtraction Infrastructure
- **src/subtract.rs**: Main subtraction logic (uses pulse.rs)
- **src/pulse.rs**: GFSK pulse generation
- **src/decoder.rs**: Multi-pass loop (lines 277-358)

### New (Unused) Infrastructure
- **src/sync/synthesize.rs**: Alternative synthesis (not used yet)

## Conclusion

We successfully enabled multi-pass decoding, but the subtraction component isn't working (0 dB power changes). The infrastructure exists - we need to debug why pulse synthesis and subtraction aren't matching the actual signals well enough to remove them.

The fact that we have TWO pulse synthesis modules (pulse.rs and synthesize.rs) suggests we need to consolidate and test which one works better.

Current progress: **7/22 decodes (32%)** vs WSJT-X's 22/22 (100%). Subtraction should improve this to 15-20/22 once working correctly.
