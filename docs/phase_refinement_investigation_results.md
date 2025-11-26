# Phase-Based Frequency Refinement Investigation - 2025-11-25

## Summary

Implemented and tested phase-based frequency refinement using Costas array phase progression. **Result: NO improvement** in decode count (still 8/22). Phase measurements capture only ~20% of actual frequency error.

---

## Implementation

### Algorithm

Measure phase of Costas arrays at three positions (symbols 0-6, 36-42, 72-78) and calculate frequency offset from phase drift:

```
Δf = Δφ / (2π × Δt)
```

Where:
- Δφ = phase difference between Costas arrays
- Δt = time between Costas arrays (36 symbols × 0.16s = 5.76s)

### Code Added

1. **[src/sync/extract.rs](../src/sync/extract.rs:967-1117)**: `estimate_frequency_from_phase()`
   - Downsamples signal at candidate frequency
   - Extracts phase from each Costas array (7 tones per array)
   - Power-weighted average phase per array
   - Selects best Costas pair (prefer 7/7 valid tones, maximum separation)
   - Calculates frequency offset from phase drift

2. **[src/decoder.rs](../src/decoder.rs:124-148)**: Integration into decode pipeline
   - Attempts phase refinement after fine sync
   - Re-extracts at refined frequency if correction is significant (> 0.01 Hz, < 1 Hz)
   - Falls back to original frequency if refinement fails

---

## Test Results

### F5RXL @ 1197 Hz (WSJT-X) vs 1196.8 Hz (RustyFt8)

**Actual frequency error**: 0.2 Hz

**Phase measurements**:
```
Costas 1 @ symbols 0-6:  phase = -0.217 rad (5/7 tones valid)
Costas 2 @ symbols 36-42: phase = -0.639 rad (7/7 tones valid)
Costas 3 @ symbols 72-78: phase = +0.732 rad (7/7 tones valid)
```

**Phase drift (Costas 2 → 3, best pair)**:
```
Δφ = +0.732 - (-0.639) = +1.372 rad over 36 symbols (5.76s)
Δf = 1.372 / (2π × 5.76) = 0.038 Hz
```

**Refinement result**: +0.038 Hz correction (only 19% of actual 0.2 Hz error!)

**Decode result**: Still fails (8/22 total, F5RXL not decoded)

---

## Root Cause Analysis

### Why Phase Refinement Doesn't Work for FT8

**Problem**: Averaging phase across different tones distorts the measurement

FT8 uses 8-FSK modulation with Costas pattern [3,1,4,0,6,5,2]:
- Tone 0: 0 Hz offset from baseband
- Tone 1: 6.25 Hz offset
- Tone 2: 12.5 Hz offset
- Tone 3: 18.75 Hz offset
- ... (6.25 Hz spacing)

When measuring phase, we average across 7 different tones in each Costas array. Each tone has:
1. **Time-dependent phase**: φ(t) = 2π × Δf × t (what we want to measure)
2. **Frequency-dependent phase**: φ(f) = 2π × f_tone × t_symbol (unwanted offset)

The frequency-dependent phases don't cancel when averaging across different tones, introducing measurement errors that reduce the apparent frequency offset.

### Expected vs Measured Phase Drift

**Expected** (for 0.2 Hz offset over 5.76s):
```
Δφ = 2π × 0.2 × 5.76 = 7.24 rad
Wrapped: 7.24 - 2π = 0.96 rad
```

**Measured**: 1.372 rad (43% higher than expected)

The 43% error comes from:
1. Averaging across different tone frequencies
2. Noise in weak signal (-2 dB SNR)
3. Missing tones in Costas 1 (negative time offset → first 2 tones skipped)

---

## Comparison with Existing Approaches

### What WSJT-X Does

WSJT-X achieves sub-0.5 Hz accuracy through:
1. **Coarse sync**: 2.93 Hz/bin resolution (4096-point FFT)
2. **Fine sync**: Correlation-based search at multiple frequency offsets (±2.5 Hz in 0.5 Hz steps)
3. **Parabolic interpolation**: 3-point fit to refine frequency from sync correlation peak

**No phase-based refinement** - WSJT-X doesn't use Costas phase progression for frequency estimation.

### What RustyFt8 Currently Does

After Part 9 (sync2d fix):
1. Coarse sync: Finds F5RXL candidates at 1195.3 Hz, 1201.2 Hz
2. Fine sync: Refines to 1196.8 Hz (0.2 Hz error), 1198.7 Hz (1.7 Hz error)
3. Phase refinement: +0.038 Hz → 1196.84 Hz (still 0.16 Hz error)
4. Extraction: nsync=19/21, mean_abs_LLR=2.27 (excellent quality)
5. LDPC: Fails due to ~20% tone errors → ~28% bit error rate

**Bottleneck remains**: 0.16-0.2 Hz frequency error → 20% tone errors → exceeds LDPC's ~20% correction capability

---

## Lessons Learned

### Phase-Based Methods Work Best For:
- ✅ Continuous single-tone signals
- ✅ BPSK/QPSK modulation
- ✅ High SNR (>10 dB)
- ✅ Long observation times

### Phase-Based Methods Struggle With:
- ❌ Frequency-hopping modulation (FT8's 8-FSK)
- ❌ Short symbol duration (0.16s)
- ❌ Low SNR (-2 dB for F5RXL)
- ❌ Averaging across different tones

---

## Alternative Approaches to Consider

### Option 1: Finer Frequency Search Grid ⭐ MOST PROMISING

**Current**: Fine sync tests ±2.5 Hz in 0.5 Hz steps (11 frequencies)
**Proposed**: ±2.5 Hz in 0.25 Hz steps (21 frequencies)

**Pros**:
- Direct solution - find 1197.0 Hz instead of 1196.8 Hz
- No algorithm changes needed
- 2x compute (acceptable)

**Cons**:
- Still discrete (can miss exact frequency between grid points)
- Doesn't address interpolation limitations

**Expected outcome**: Reduce error from 0.2 Hz to ~0.1 Hz → ~10% tone errors → ~15% bit errors → within LDPC capability

### Option 2: Wider FFT Bins for Tone Extraction

**Current**: 32-point FFT → 0.195 Hz/bin resolution
**Proposed**: 64-point FFT → 0.098 Hz/bin resolution

**Pros**:
- More robust to frequency errors
- 0.2 Hz error = 2 bins (vs current 1 bin)
- Less energy leakage to adjacent bins

**Cons**:
- 2x more compute for extraction
- Slight loss in time resolution
- Doesn't fix frequency estimation, just makes it more tolerant

**Expected outcome**: Tolerate 0.2-0.4 Hz errors → decode F5RXL at 1196.8 Hz

### Option 3: Iterative Fine Sync Refinement

Start with coarse frequency, extract Costas, use Costas quality to guide finer search:
1. Initial fine sync → 1196.8 Hz
2. Extract Costas → measure quality (nsync, power)
3. Test frequencies around 1196.8 Hz in 0.1 Hz steps
4. Select frequency with best Costas quality
5. Re-extract and decode

**Pros**:
- Uses actual signal quality as metric
- Can achieve arbitrary precision

**Cons**:
- 5-10x more compute (multiple extractions per candidate)
- Complex implementation

**Expected outcome**: Sub-0.1 Hz accuracy, but at significant computational cost

### Option 4: Machine Learning Frequency Estimation

Train a neural network to estimate frequency offset from spectrogram or downsampled signal.

**Pros**:
- Could learn optimal features automatically
- Might handle multi-tone signals better

**Cons**:
- Requires training data
- Computationally expensive
- Black box (hard to debug)

**Expected outcome**: Unknown - experimental

---

## Recommendations

### Priority 1: Finer Frequency Search Grid (0.25 Hz steps)

**Effort**: Low (change one constant)
**Compute**: 2x fine sync (acceptable)
**Expected improvement**: +3-5 decodes (11-13/22, 50-59%)

**Implementation**:
```rust
// In src/sync/fine.rs
const FINE_FREQ_STEP: f32 = 0.25; // Changed from 0.5
const FINE_FREQ_RANGE: f32 = 2.5; // Keep at ±2.5 Hz
```

### Priority 2: Wider FFT Bins (64-point FFT)

**Effort**: Medium (change FFT size, adjust tone mapping)
**Compute**: 2x extraction (acceptable)
**Expected improvement**: +2-4 decodes (10-12/22, 45-55%)

### Priority 3: Combination (0.25 Hz + 64-point FFT)

**Effort**: Medium
**Compute**: 4x total (2x fine sync × 2x extraction)
**Expected improvement**: +5-10 decodes (13-18/22, 59-82%)

---

## Conclusion

**Phase-based frequency refinement**: ❌ Not effective for FT8
- Implemented successfully but captures only ~20% of frequency error
- Root cause: Multi-tone modulation breaks phase measurement assumptions
- WSJT-X doesn't use this approach (for good reason)

**Current status**: 8/22 decodes (36%)
- Sync2d fixed ✅
- Frequency estimation accuracy: 0.2-1.7 Hz (needs improvement)
- LDPC fails at 0.2 Hz error due to 20% tone errors

**Next steps**: Implement finer frequency search grid (0.25 Hz steps)
- Expected: 11-13/22 decodes (50-59%)
- Simple, low-risk, proven approach
- Addresses root cause directly

---

## Files Modified

- [src/sync/extract.rs](../src/sync/extract.rs): Added `estimate_frequency_from_phase()` (lines 967-1117)
- [src/sync/mod.rs](../src/sync/mod.rs): Exported new function (line 39)
- [src/decoder.rs](../src/decoder.rs): Integrated phase refinement (lines 124-148)

## Performance Impact

- Phase measurement: 21 FFTs (3 Costas × 7 tones)
- Overhead: ~2-3% per candidate
- Negligible impact on total decode time

---

## References

- [phase_based_refinement_plan.md](phase_based_refinement_plan.md) - Original implementation plan
- [f5rxl_final_bottleneck_analysis.md](f5rxl_final_bottleneck_analysis.md) - Root cause analysis
- [tone_extraction_root_cause.md](tone_extraction_root_cause.md) - Why 0.2 Hz causes 20% errors
- [sync2d_fix_breakthrough_20251125.md](sync2d_fix_breakthrough_20251125.md) - Sync2d fix details
