# OSD False Positive Investigation - 2025-11-25

## Summary

Investigated false positives from OSD (Ordered Statistics Decoding) in multi-pass decoding. Found that OSD decodes are responsible for most signals, including both correct decodes and false positives. Implemented optional OSD filtering but discovered trade-offs.

## Background

After fixing the time offset bug and getting multi-pass working (9/22 decodes), noticed a false positive in Pass 2:
- **Our decode**: "J9BFQ ZM5FEY R QA56" @ 2695 Hz (-17 dB)
- **WSJT-X**: "K1BZM EA3GP -09" @ 2695 Hz (-3 dB)

## Investigation

### LDPC Decoder Usage

Added diagnostic output to track LDPC iteration counts:
- `iters > 0`: Belief propagation (BP) converged
- `iters == 0`: BP failed, OSD was used

### Pass 1 Results (7 decodes)

| Signal | SNR | Decoder | Status |
|--------|-----|---------|--------|
| W1FC @ 2572 Hz | -8 dB | **OSD** | ✓ Correct |
| XE2X @ 2854 Hz | -14 dB | **OSD** | ✓ Correct |
| N1API @ 2238 Hz | -12 dB | **BP** (16 iters) | ✓ Correct |
| WM3PEN @ 2157 Hz | -4 dB | **OSD** | ✓ Correct |
| K1JT @ 589 Hz | -14 dB | **BP** (4 iters) | ✓ Correct |
| W1DIG @ 2733 Hz | -11 dB | **BP** (6 iters) | ✓ Correct |
| N1JFU @ 642 Hz | -15 dB | **BP** (2 iters) | ✓ Correct |

**Key finding**: Even strong signals (-4 to -8 dB) are using OSD! This suggests LLR quality is insufficient for BP convergence.

### Pass 2 Results (1 decode)

| Signal | SNR | Decoder | Status |
|--------|-----|---------|--------|
| 2695 Hz | -17 dB | **OSD** | ✗ False positive |

### Pass 3 Results (1 decode)

| Signal | SNR | Decoder | Status |
|--------|-----|---------|--------|
| 398.9 Hz | -16 dB | **BP** (8 iters) | ✓ Correct |

**Important**: This signal should have been found in Pass 1 (WSJT-X finds it @ 400 Hz, -16 dB). Suggests coarse sync issues.

## Root Cause

### Why OSD is Used So Much

1. **Poor LLR quality**: Our LLR values are too weak for BP to converge
2. **Normalization issues**: Mean absolute LLR is ~2.2, max ~7-11
3. **Tone extraction errors**: Wrong bins have higher power (in-band interference)
4. **Lack of nsym=2/3**: No multi-symbol coherent combining to improve SNR

### Why OSD Creates False Positives

1. **OSD order 4**: Can correct many bit errors, but also finds valid codewords in noise
2. **No confidence measure**: OSD doesn't provide reliability indication
3. **Weak signals**: At -17 dB SNR, signal is buried in noise floor
4. **After subtraction**: Pass 2+ audio has subtraction artifacts making noise look like signal

## Attempted Solutions

### Option 1: Filter OSD Decodes by SNR

```rust
// Filter OSD decodes below -15 dB SNR
if iters == 0 && snr_db < -15 {
    continue; // Likely false positive
}
```

**Result**: Filters false positive, but also stops Pass 3 from running!
- Pass 2 finds 0 new messages (false positive filtered)
- Multi-pass loop stops ("No new signals found")
- Pass 3 never runs, missing the 398.9 Hz correct decode
- **7/22 decodes** vs 9/22 without filter

### Option 2: Relax Threshold to -12 dB

**Same issue**: False positive at -17 dB still filtered, Pass 3 still doesn't run.

### Option 3: Disable OSD Filter (Current)

**Accepted trade-off**: 9/22 decodes with 1 known false positive
- Prioritizes recall (finding all signals) over precision (no false positives)
- False positive rate: 1/9 (11%)
- WSJT-X achieves 0 false positives with 22/22 decodes

## The Pass 3 Problem

**Why does 398.9 Hz only appear in Pass 3?**

1. **Coarse sync limitation**: No candidate found near 400 Hz in Pass 1
   - Closest candidates: 430.7, 442.4, 454.1, 468.8 Hz
   - 30-70 Hz away from actual signal @ 400 Hz

2. **Sync threshold too high**: Weak signals don't generate strong sync peaks

3. **Accidental benefit**: Pass 2 false positive "accidentally" allows Pass 3 to run
   - Without it, multi-pass stops at Pass 2

## Comparison with WSJT-X

### WSJT-X (22/22 decodes, 0 false positives)

- Finds 400 Hz signal in first pass
- Better coarse sync algorithm
- Better LLR quality (BP converges more often)
- More conservative OSD usage

### RustyFt8 (9/22 decodes, 1 false positive)

- Misses 400 Hz signal in Pass 1 and Pass 2
- Coarse sync doesn't find weak signals as well
- Poor LLR quality forces heavy OSD usage
- OSD used for 4/7 Pass 1 decodes (57%)

## Root Issues

### 1. Coarse Sync Not Finding Weak Signals

**Evidence**: 400 Hz signal completely missed in Pass 1, only found in Pass 3
**Impact**: Miss initial candidates, rely on accidental Pass 3 execution
**Solution**: Improve coarse sync sensitivity or lower threshold

### 2. Poor LLR Quality

**Evidence**: Strong signals (-4 to -8 dB) requiring OSD instead of BP
**Impact**: Forces OSD usage, increases false positive rate
**Solution**:
- Improve tone extraction (reduce in-band interference)
- Enable nsym=2/3 multi-symbol combining
- Better LLR normalization

### 3. Multi-Pass Stopping Criteria Too Aggressive

**Evidence**: Pass 3 only runs if Pass 2 finds something
**Impact**: Weak signals discoverable in later passes are missed
**Solution**: Always run minimum 2-3 passes regardless of results

## Recommended Actions

### Priority 1: Improve LLR Quality

**Goal**: Reduce OSD usage from 57% to <20%

Options:
1. Enable nsym=2/3 multi-symbol combining (3-6 dB SNR improvement)
2. Better tone extraction (fix in-band interference issues)
3. Improved LLR normalization per symbol
4. Phase tracking from Costas arrays

**Expected impact**: More BP convergence, fewer false positives

### Priority 2: Improve Coarse Sync

**Goal**: Find weak signals like 400 Hz in Pass 1

Options:
1. Lower sync threshold for top N candidates
2. Try multiple threshold values
3. Improve sync algorithm (better match to WSJT-X)
4. Add sync refinement stage

**Expected impact**: +3-5 additional signals found

### Priority 3: Refine Multi-Pass Strategy

**Goal**: Don't rely on false positives to continue passes

Options:
1. Always run minimum N passes (e.g., 3) regardless of results
2. Use different stopping criteria (e.g., diminishing returns)
3. Track subtraction effectiveness, stop if <5% signals subtract
4. Separate pass continuation from signal count

**Expected impact**: More robust, predictable behavior

### Priority 4: False Positive Filtering

**Goal**: Reduce false positives without losing correct decodes

Options:
1. Use pass-aware OSD thresholds (stricter in Pass 2+)
2. Check message plausibility (callsign databases, grid squares)
3. Require minimum subtraction effectiveness for Pass 2+ candidates
4. Use LDPC confidence metrics (not just iters==0)

**Expected impact**: <5% false positive rate

## Current Status

**Configuration**: OSD filter disabled
**Performance**: 9/22 decodes (41%) vs WSJT-X 22/22 (100%)
**False positives**: 1/9 (11%)
**Trade-off**: Accepting false positives to maintain recall

**Decision**: Prioritize finding more signals over eliminating false positives. Once we improve LLR quality and coarse sync to match WSJT-X's 22/22, we can add stricter filtering.

## Files Modified

- [src/decoder.rs](../src/decoder.rs): Added LDPC iteration logging and optional OSD filter (disabled)

## Key Code Changes

```rust
// Line 179-188: LDPC iteration diagnostics (disabled)
let _debug_ldpc = false;
if _debug_ldpc {
    let decode_type = if iters == 0 { "OSD" } else { "BP" };
    eprintln!("  LDPC: {} iters={}, freq={:.1} Hz, nsym={}, scale={:.1}",
             decode_type, iters, refined.frequency, nsym, scale);
}

// Line 230-237: Optional OSD filtering (disabled)
let _enable_osd_filter = false;
if _enable_osd_filter && iters == 0 && snr_db < -15 {
    continue; // Filter weak OSD decodes
}
```

## Lessons Learned

1. **OSD is a double-edged sword**: Enables weak signal decoding but creates false positives
2. **Multi-pass dependencies**: Filtering one pass affects subsequent passes
3. **LLR quality matters**: Poor LLRs force OSD usage even for strong signals
4. **Coarse sync is critical**: Missing candidates in Pass 1 cascades through all passes
5. **Trade-offs required**: Can't optimize both precision and recall simultaneously at current quality level

## Next Session Focus

1. **Improve LLR quality** to reduce OSD dependency
2. **Improve coarse sync** to find weak signals in Pass 1
3. **Consider nsym=2/3** for 3-6 dB SNR improvement
4. **Document false positive rate** as known issue until LLR/sync improved
