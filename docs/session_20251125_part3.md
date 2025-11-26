# Session Summary - 2025-11-25 Part 3: OSD False Positive Investigation

## Overview

Continued from [session_20251125_summary.md](session_20251125_summary.md) after successfully implementing multi-pass decoding. Investigated false positives and discovered that OSD (Ordered Statistics Decoding) is being used for most decodes, including strong signals.

## Key Findings

### 1. OSD Dominance

**Discovery**: Added LDPC iteration count logging and found OSD is used for **4/7 Pass 1 decodes (57%)**!

Even strong signals like:
- W1FC @ 2572 Hz (-8 dB): **OSD** ← Should use BP!
- WM3PEN @ 2157 Hz (-4 dB): **OSD** ← Strong signal!

This indicates **poor LLR quality** preventing BP convergence.

### 2. False Positive Mechanism

**Pass 2 false positive**:
- Decoded: "J9BFQ ZM5FEY R QA56" @ 2695 Hz (-17 dB)
- Correct: "K1BZM EA3GP -09" @ 2695 Hz (-3 dB)
- Used: **OSD** (iters=0)

OSD can decode noise into any valid message, creating false positives.

### 3. The Pass 3 Problem

**Pass 3 correct decode**:
- 398.9 Hz "W0RSJ EA3BMU RR73" (-16 dB)
- Used: **BP** (8 iterations)
- **WSJT-X finds this in first pass @ 400 Hz**

Why only in Pass 3?
- **Coarse sync doesn't find candidate near 400 Hz in Pass 1**
- Closest candidates: 430.7, 442.4, 454.1, 468.8 Hz (30-70 Hz away)
- Only becomes visible after multiple passes

### 4. Filtering Trade-off

**Attempted OSD filter** (threshold: -15 dB):
```rust
if iters == 0 && snr_db < -15 {
    continue; // Filter likely false positives
}
```

**Result**:
- ✓ Filters false positive @ 2695 Hz (-17 dB)
- ✗ Pass 2 finds 0 new messages
- ✗ Multi-pass stops ("No new signals found")
- ✗ Pass 3 never runs, losing 398.9 Hz correct decode
- **7/22 decodes** instead of 9/22

**Decision**: Disable OSD filter, accept 1 false positive to maintain 9 decodes.

## Root Cause Analysis

### Why OSD is Overused

1. **Poor LLR quality**:
   - Mean absolute LLR: ~2.2 (should be higher)
   - Max LLR: ~7-11 (marginal)
   - Insufficient confidence for BP convergence

2. **Tone extraction errors**:
   - In-band interference (e.g., W1DIG @ 2733 Hz interferes with K1BZM @ 2695 Hz)
   - Wrong bins have higher power than correct bins
   - 81% tone accuracy (need 90%+)

3. **No multi-symbol combining**:
   - nsym=2/3 disabled (would provide 3-6 dB SNR improvement)
   - Single-symbol extraction noisier

4. **Weak signals**:
   - At -12 to -15 dB SNR, signals are marginally above noise
   - BP requires cleaner input to converge

### Why Coarse Sync Misses Weak Signals

1. **Sync threshold too high**:
   - 400 Hz signal not found as candidate
   - Only appears in Pass 3 after multiple subtractions

2. **Algorithm differences**:
   - WSJT-X finds 400 Hz in first pass
   - Our coarse sync less sensitive to weak signals

3. **No candidate refinement**:
   - Could try multiple thresholds
   - Could use iterative refinement

## Comparison with WSJT-X

| Metric | RustyFt8 | WSJT-X | Gap |
|--------|----------|--------|-----|
| **Total decodes** | 9/22 (41%) | 22/22 (100%) | -59% |
| **False positives** | 1/9 (11%) | 0/22 (0%) | +11% |
| **OSD usage** | 4/7 (57%) | ~10-20% | +37-47% |
| **400 Hz signal** | Pass 3 | Pass 1 | 2 passes late |
| **Effective subtraction** | 2/7 (29%) | ~6/7 (86%) | -57% |

## Code Changes

### src/decoder.rs

**Added LDPC iteration diagnostics** (lines 178-188):
```rust
// Debug: log LDPC decoder type (disabled by default)
let _debug_ldpc = false;
if _debug_ldpc {
    let decode_type = if iters == 0 { "OSD" } else { "BP" };
    eprintln!("  LDPC: {} iters={}, freq={:.1} Hz, nsym={}, scale={:.1}",
             decode_type, iters, refined.frequency, nsym, scale);
}
```

**Added optional OSD filtering** (lines 227-237, DISABLED):
```rust
// Additional filtering for OSD decodes (iters==0)
// DISABLED: This filters false positives but also stops Pass 3 from running
let _enable_osd_filter = false;
if _enable_osd_filter && iters == 0 && snr_db < -15 {
    continue; // Filter likely false positives
}
```

## Documentation Created

1. **[osd_false_positive_investigation.md](osd_false_positive_investigation.md)**: Complete investigation with root cause analysis
2. **[session_20251125_part3.md](session_20251125_part3.md)**: This document

## Current Status

**Performance**: 9/22 decodes (41%) vs WSJT-X 22/22 (100%)
**False positive rate**: 1/9 (11%)
**OSD usage**: 4/7 Pass 1 decodes (57%)
**Configuration**: OSD filter disabled

**Trade-off accepted**: Prioritizing recall (finding signals) over precision (no false positives) until LLR quality improves.

## Next Steps

### Priority 1: Improve LLR Quality (Highest Impact)

**Goal**: Reduce OSD usage from 57% to <20%

Strong signals should use BP, not OSD! This indicates fundamental LLR issues.

**Options**:
1. **Enable nsym=2/3 multi-symbol combining** (3-6 dB SNR improvement)
   - Implement per-symbol phase tracking from Costas arrays
   - Average 2-3 symbols coherently
   - Would help weak signals (-15 to -24 dB)

2. **Improve tone extraction**:
   - Better handling of in-band interference
   - More aggressive subtraction
   - Frequency-domain filtering improvements

3. **Better LLR normalization**:
   - Per-symbol normalization
   - Account for SNR variations across transmission
   - Use baseline noise measurements

4. **Phase tracking**:
   - Use Costas array phase estimates
   - Correct phase drift during extraction
   - Improve coherent combining

**Expected impact**: +5-10 signals, OSD usage <20%, fewer false positives

### Priority 2: Improve Coarse Sync

**Goal**: Find weak signals like 400 Hz in Pass 1 (not Pass 3)

**Options**:
1. Lower sync threshold for top N candidates
2. Try multiple threshold values (adaptive)
3. Improve sync algorithm (match WSJT-X more closely)
4. Add candidate refinement stage
5. Better Costas array matching

**Expected impact**: +3-5 signals found earlier, more robust

### Priority 3: Refine Multi-Pass Strategy

**Goal**: Don't depend on false positives for pass continuation

**Options**:
1. Always run minimum 3 passes regardless of results
2. Use diminishing returns as stopping criteria
3. Track subtraction effectiveness
4. Separate continuation logic from new message count

**Expected impact**: More predictable behavior, catch signals in later passes

### Priority 4: False Positive Filtering (After LLR Improvements)

**Goal**: <5% false positive rate without losing correct decodes

**Options**:
1. Pass-aware OSD thresholds (stricter in Pass 2+)
2. Message plausibility checks (callsign databases)
3. Require subtraction effectiveness for Pass 2+ candidates
4. LDPC confidence metrics (beyond just iters==0)

**Expected impact**: Eliminate false positives once LLR quality improves

## Technical Insights

### OSD as a Symptom, Not Root Cause

OSD creating false positives is a **symptom** of poor LLR quality. The real issue:
- Strong signals (-4 to -8 dB) shouldn't need OSD
- BP should converge with good LLRs
- OSD is a fallback for marginal cases

**Solution**: Fix LLR quality, not just filter OSD results.

### Multi-Pass Dependencies

Filtering in earlier passes affects later passes:
- Filter Pass 2 → Pass 2 finds 0 → Pass 3 doesn't run
- Must consider whole pipeline, not individual passes
- WSJT-X avoids this by finding signals in Pass 1

### The 400 Hz Mystery

WSJT-X finds 400 Hz in first pass, we find it in Pass 3 (if at all):
- **Not a decoder issue** (we decode it correctly when found)
- **Sync/candidate selection issue** (we don't look there)
- **Cascading problem** (miss in Pass 1 → rely on Pass 3)

## Lessons Learned

1. **Diagnostics reveal hidden issues**: OSD usage much higher than expected
2. **Strong signals are diagnostic**: If -4 dB signal needs OSD, LLRs are bad
3. **Multi-pass is complex**: Interactions between passes not obvious
4. **False positives have context**: Some help (trigger Pass 3), some hurt (waste effort)
5. **WSJT-X sets high bar**: 100% decode rate, 0% false positives, minimal OSD usage

## Conclusion

Discovered that OSD is being heavily overused (57% of Pass 1 decodes), even for strong signals. This indicates **poor LLR quality** is the root cause of both:
- Low decode rate (9/22 vs 22/22)
- False positives (OSD decoding noise)

**Current state**: 9/22 decodes (41%) with 1 false positive (11% rate)
**Next priority**: Improve LLR quality through nsym=2/3, phase tracking, and better tone extraction
**Expected**: Match WSJT-X's 22/22 decode rate with <5% false positive rate

The foundation (multi-pass, subtraction) is working. We need to improve the signal quality (LLRs) and candidate selection (coarse sync) to match WSJT-X performance.

## Time Investment

**Session 3 time**: ~1.5 hours
- LDPC diagnostics: ~30 minutes
- OSD filter attempts: ~30 minutes
- Investigation and analysis: ~30 minutes

**Cumulative session time**: ~5.5-7 hours (Parts 1, 2, and 3)

**Result**: Identified root cause (LLR quality), clear path forward (nsym=2/3 + phase tracking).
