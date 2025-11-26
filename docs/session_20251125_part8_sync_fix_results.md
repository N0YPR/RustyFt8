# Session Part 8: Sync Score Preservation Fix - 2025-11-25

## Summary

Implemented fix to preserve coarse sync scores (matching WSJT-X architecture) but still at **8/22 decodes** because the real bottleneck is **fine sync frequency inaccuracy** causing tone extraction errors.

## What Was Fixed

### Issue: Fine Sync Replacing Coarse Sync Scores

**Discovery**: WSJT-X preserves coarse sync scores for candidate ranking:
- `sync8.f90` line 124: Stores coarse sync in `candidate0(3,k)=red(n)`
- `ft8d.f90` line 49: Uses coarse sync for ranking: `sync=candidate(3,icand)`
- `ft8b.f90` lines 110-152: Computes fine sync internally but **does NOT return it**

RustyFt8 was **replacing** good coarse sync scores with lower fine sync scores, causing negative DT signals to be filtered out.

**Fix Applied**: [src/sync/fine.rs](../src/sync/fine.rs#L252)
```rust
Ok(Candidate {
    frequency: best_freq,           // Refined frequency
    time_offset: refined_time,      // Refined time
    sync_power: candidate.sync_power,  // ← PRESERVE coarse sync score
    baseline_noise: candidate.baseline_noise,
})
```

### Results After Fix

**Decode count**: Still 8/22 (no improvement)

**F5RXL status**:
- ✅ Coarse sync finds it: freq=1195.3 Hz, sync=3.157
- ✅ Fine sync refines it: freq=1196.8 Hz, dt=-0.77s
- ✅ Extraction works: nsync=19/21 (90% Costas symbols found!)
- ✅ LLR computed: mean=2.27, max=5.73
- ✅ LDPC tries 29 attempts (both diff and ratio methods, multiple scales)
- ❌ **BP converges quickly (2-3 iters) but produces INVALID codewords**

## Root Cause: Fine Sync Frequency Inaccuracy

From [tone_extraction_root_cause.md](tone_extraction_root_cause.md):

### The Problem

Fine sync searches in **0.5 Hz discrete steps**, but FT8 needs **<0.1 Hz accuracy** to avoid FFT bin smearing.

**Example: F5RXL**
- WSJT-X decodes at: **1197.0 Hz**
- Coarse sync finds: 1195.3 Hz (1.7 Hz off)
- Fine sync refines to: **1196.8 Hz** (0.2 Hz off)
- Error: 0.2 Hz = **1 FFT bin** at 0.195 Hz/bin resolution

### Why 0.2 Hz Matters

FT8 uses 8-FSK with 6.25 Hz tone spacing:
- FFT resolution: 6.25 Hz / 32 bins = **0.195 Hz/bin**
- 0.2 Hz offset = **1 FFT bin shift**
- Wrong tone bins receive more power than correct bins
- Result: 20% tone extraction errors (exceeds LDPC correction capability)

### Extraction Working But LLRs Wrong

F5RXL extraction shows:
- **nsync=19/21** (90% Costas symbols correct)
- But data symbols have ~20% tone errors
- Each tone error = 3 bit errors (tone encodes 3 bits)
- Effective bit error rate: **~28%** (exceeds LDPC's ~20% max)
- BP converges quickly but to WRONG codeword

## Why Preserving Sync Score Didn't Help

The sync score preservation only affects **candidate ranking** (which candidates get LDPC attempts). It doesn't fix the **frequency accuracy** used for tone extraction.

F5RXL was already ranked ~50th (within top 150), so it was already being extracted and decoded. The issue is that extraction at **1196.8 Hz produces wrong tones**, not that it wasn't being tried.

## What We Learned

### ✅ Things Working Correctly

1. **Coarse sync**: Finds all signals (F5RXL found at 1195.3 Hz)
2. **Time offset handling**: Negative DT signals extract correctly (nsync=19/21)
3. **Fine sync time refinement**: dt=-0.77s vs WSJT-X's -0.8s (0.03s accuracy ✓)
4. **Fine sync frequency refinement**: 1195.3→1196.8 Hz (improved from 1.7 Hz to 0.2 Hz error)
5. **Extraction**: Per-symbol bounds checking works, Costas arrays found
6. **LDPC attempts**: Dual LLR methods tried with multiple scales
7. **BP convergence**: Fast convergence (2-3 iters) shows LLR confidence is high

### ❌ Remaining Bottleneck

**Fine sync frequency accuracy**: 0.5 Hz discrete steps → 0.2 Hz typical error → tone extraction errors → wrong codewords

## Why Fine Sync Can't Hit 1197 Hz Exactly

Fine sync searches ±2.5 Hz in 0.5 Hz steps:
- Tests: [..., 1195.8, 1196.3, **1196.8**, 1197.3, 1197.8, ...]
- Actual signal: **1197.0 Hz** (between test frequencies)
- Best match by sync power: 1196.8 Hz (closest discrete test)
- Error: 0.2 Hz (can't avoid with 0.5 Hz steps!)

## Solution: Sub-Bin Frequency Interpolation

From tone_extraction_root_cause.md proposed solutions:

### Option 1: Sub-Bin Interpolation (Recommended)

After discrete frequency search, interpolate sync power curve to find true peak:
1. Find best_freq at discrete steps (currently done)
2. Get sync scores at [best_freq-0.5, best_freq, best_freq+0.5]
3. Fit parabola to 3 points
4. Find parabola peak (sub-bin frequency estimate)
5. Use interpolated frequency for extraction

**Expected accuracy**: 0.05-0.1 Hz (5-10x better than current 0.5 Hz quantization)

**Expected impact**:
- Reduce tone errors from 20% to 5-10%
- Enable LDPC to decode (within 20% correction capability)
- **Target: +5-10 new decodes** (12-18/22 total)

### Option 2: Finer Search Grid

Reduce step size from 0.5 Hz to 0.1 Hz:
- Tests: 51 frequencies instead of 11 (5x more compute)
- Still discrete (can't hit exact frequency)
- Diminishing returns vs Option 1

### Option 3: Iterative Refinement

After initial extraction, use decoded Costas tones to refine frequency:
- Measure Costas phase progression
- Calculate precise frequency offset
- Re-extract at corrected frequency
- More complex, requires phase unwrapping

## Current Architecture Summary

```
Coarse Sync (sync8.f90 equiv)
  ↓
  Finds candidates with sync scores
  ↓
Fine Sync (ft8b.f90 equiv) - [src/sync/fine.rs]
  ↓
  Frequency search: ±2.5 Hz, 0.5 Hz steps  ← BOTTLENECK
  Time search: ±10 lag steps
  ↓
  Returns: (refined_freq ± 0.2 Hz, refined_time ± 0.01s, coarse_sync_score) ✓
  ↓
Extraction (ft8b.f90 equiv) - [src/sync/extract.rs]
  ↓
  Downsample at refined_freq ± 0.2 Hz error
  Extract tones via FFT (0.195 Hz/bin resolution)
  ↓
  Result: 20% tone errors due to 0.2 Hz ≈ 1 bin offset
  ↓
LLR Computation - [src/sync/extract.rs]
  ↓
  Dual methods (diff + ratio), normalized, scaled
  ↓
  Result: High confidence (mean=2.27) but WRONG bits
  ↓
LDPC Decode - [src/decoder.rs]
  ↓
  BP converges quickly (2-3 iters) to WRONG codeword
  ↓
❌ Validation fails (wrong callsigns or CRC error)
```

## Next Steps

### Priority 1: Implement Sub-Bin Frequency Interpolation ⚠️ CRITICAL

**Goal**: Improve fine sync accuracy from 0.2 Hz to 0.05 Hz

**Implementation** in [src/sync/fine.rs](../src/sync/fine.rs):
1. After frequency search loop (line 211), save sync scores:
   ```rust
   let mut sync_scores = vec![(f32, f32)]; // (freq, sync) pairs
   ```
2. Fit parabola to top 3 points around best_freq
3. Find parabola peak: `refined_freq = parabolic_interpolate(sync_scores)`
4. Use refined_freq for final extraction

**Expected result**: F5RXL decodes at ~1197.0 Hz (0.0 Hz error)

**Expected impact**: +5-10 decodes (12-18/22 total, 55-82%)

### Priority 2: Test Interpolation on Known Failures

Test signals with known frequencies from WSJT-X output:
- F5RXL @ 1197 Hz (currently 1196.8 Hz)
- K1BZM @ 2695 Hz (tone analysis doc shows 0.3 Hz error)
- DL8YHR @ 2610 Hz

Verify interpolation brings frequency error < 0.1 Hz for all.

### Priority 3: Handle Edge Cases

- Interpolation near band edges (can't test freq-0.5 or freq+0.5)
- Flat sync power curves (low SNR signals)
- Multiple peaks (interference)

## Files Modified This Session

- [src/sync/fine.rs](../src/sync/fine.rs#L245-L254): Preserve coarse sync score

## Files To Modify Next

- [src/sync/fine.rs](../src/sync/fine.rs): Add parabolic interpolation for sub-bin frequency
- [docs/NEXT_STEPS.md](../NEXT_STEPS.md): Update with interpolation priority

## References

- [tone_extraction_root_cause.md](tone_extraction_root_cause.md): Root cause analysis of frequency inaccuracy
- [dual_llr_results_20251125.md](dual_llr_results_20251125.md): Shows strong signals need better extraction
- WSJT-X ft8b.f90 lines 110-151: Fine sync reference implementation
- WSJT-X sync8.f90 lines 122-124: Coarse sync score preservation

## Status

- ✅ Time offset clipping bug fixed
- ✅ Sync normalization for negative DT fixed
- ✅ Sync score preservation implemented
- ❌ **Still 8/22 decodes** - frequency accuracy is the remaining bottleneck
- **Next**: Implement sub-bin frequency interpolation (expect 12-18/22 decodes)
