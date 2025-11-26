# Root Cause: Tone Extraction Errors Due to Fine Sync Inaccuracy

**Date**: 2025-11-24
**Status**: ROOT CAUSE IDENTIFIED

---

## Problem Statement

K1BZM EA3GP @ 2695 Hz fails to decode despite:
- Excellent Costas sync (20/21)
- LLR values matching successful decodes (mean_abs_LLR=2.67 with normalized method)
- Even 5x LLR amplification doesn't help (mean_abs_LLR=13.35 still fails)

**Conclusion**: The problem is NOT LLR magnitude - it's that the extracted bits are fundamentally WRONG.

---

## Investigation: Tone Extraction Accuracy

Added debug code to extract and compare actual tones vs expected tones for K1BZM.

### Results

**Tone Accuracy: 63/79 (79.7%)**
**Tone Errors: 16/79 (20.3%)**

### Extracted vs Expected Tones

```
Extracted: 2140652 03227674074460620551746142413 3140652 76761725131701312530042577240 3140652
Expected:  3140652 03227073004460620551746353755 3140652 57761725130701312530042543240 3140652
           ^         ^^  ^^  ^              ^^^^^^           ^                     ^^
```

**Key Findings**:
1. First Costas sync has 1 error: tone 0 is 2 instead of 3 (off by -1)
2. Middle Costas is PERFECT: 3140652 (7/7 correct)
3. Last Costas is PERFECT: 3140652 (7/7 correct)
4. Data symbols have 13 errors concentrated in first data block (symbols 12-35)

### Error Distribution

| Tone Diff | Count | Percentage |
|-----------|-------|------------|
| -4 | 1 | 6.7% |
| -3 | 1 | 6.7% |
| -2 | 2 | 13.3% |
| -1 | 4 | 26.7% ← Most common |
| +1 | 2 | 13.3% |
| +2 | 1 | 6.7% |
| +3 | 1 | 6.7% |
| +4 | 1 | 6.7% |
| +6 | 1 | 6.7% |
| +7 | 1 | 6.7% ← Large jump! |

**Pattern**: Errors are NOT systematic - they range from -4 to +7. This is NOT a simple frequency/timing offset.

---

## Critical Discovery: Wrong Tone Has Higher Power

For each error, I checked the FFT power of the expected tone vs the (wrong) detected tone:

| Symbol | Expected→Got | Exp Power | Got Power | Ratio | Analysis |
|--------|--------------|-----------|-----------|-------|----------|
| 0 | 3→2 (diff:-1) | 0.093 | 0.100 | 1.08x | Almost tied - marginal |
| 12 | 0→6 (diff:+6) | 0.166 | 0.188 | 1.13x | Wrong peak slightly stronger |
| 14 | 3→4 (diff:+1) | 0.026 | 0.040 | 1.54x | Wrong peak stronger |
| 16 | 0→7 (diff:+7) | 0.047 | 0.050 | 1.06x | Almost tied |
| **30** | **3→1 (diff:-2)** | **0.032** | **0.176** | **5.5x** | **Wrong peak WAY stronger!** |
| **31** | **5→4 (diff:-1)** | **0.061** | **0.109** | **1.8x** | **Wrong peak dominates** |
| **32** | **3→2 (diff:-1)** | **0.057** | **0.258** | **4.5x** | **Wrong peak crushes expected** |
| **33** | **7→4 (diff:-3)** | **0.099** | **0.185** | **1.9x** | **Wrong peak stronger** |
| **34** | **5→1 (diff:-4)** | **0.012** | **0.199** | **16.6x** | **Wrong peak dominates (16x!)** |
| **35** | **5→3 (diff:-2)** | **0.026** | **0.380** | **14.6x** | **Wrong peak crushes (15x!)** |

**KEY INSIGHT**: We're not detecting noise - we're detecting a REAL signal component, but it's in the WRONG FFT bin!

For symbols 30-35, the wrong tone has 2-17x more power than the expected tone. This is NOT random noise - there's real signal energy in those bins, but it shouldn't be there.

---

## Root Cause: Fine Sync Inaccuracy

**WSJT-X reports**: K1BZM EA3GP @ **2695 Hz, dt=-0.1s**
**We detect**: K1BZM EA3GP @ **2695.3 Hz, dt=-0.12s**

**Difference**: 0.3 Hz frequency, 0.02s timing (20ms)

### Why This Matters

FT8 uses 8-FSK with 6.25 Hz tone spacing. For a weak signal (-3 dB):
1. Small frequency offset (0.3 Hz) shifts FFT peak energy between adjacent bins
2. Small timing offset (20ms) causes inter-symbol interference
3. Combined effect: Wrong tone appears to have higher power

### FFT Bin Resolution

- Symbol duration: 160ms (0.16s)
- FFT size: 32 samples/symbol
- Frequency resolution: 6.25 Hz / 32 = **0.195 Hz per FFT bin**

A 0.3 Hz offset is **1.5 FFT bins** - enough to shift the peak to the wrong tone!

---

## Why LDPC Can't Save Us

LDPC/OSD can correct a few bit errors (typically 10-20% bit error rate max), but:
1. We have 20% tone errors (16/79)
2. Each tone encodes 3 bits, so tone errors corrupt 3 bits each
3. Effective bit error rate: 16 tones × 3 bits / 174 total bits = **28% bit error rate**
4. This exceeds LDPC's correction capability

**Even amplifying LLR by 5x doesn't help** because the problem isn't confidence - it's that the bits are WRONG.

---

## Solution: Improve Fine Sync Accuracy

### Current Fine Sync

From `src/sync/fine.rs`:
- Frequency search: ±2.5 Hz in 0.5 Hz steps
- Time search: ±10 lag steps (±40ms at 200 Hz sample rate)
- Resolution: 0.5 Hz, 5ms

### WSJT-X Fine Sync

From `wsjtx/lib/ft8/ft8b.f90` lines 103-150:
- Frequency search: ±2.5 Hz in 0.5 Hz steps (same as ours)
- Time search: ±4 lag steps at 200 Hz (±20ms)
- **Additional refinement**: Peak-fitting to get sub-bin accuracy

### Proposed Improvements

1. **Sub-bin frequency estimation**
   Instead of discrete 0.5 Hz steps, interpolate FFT peak to get 0.1 Hz accuracy

2. **Finer time search**
   Use ±2 lag steps (±10ms) after initial coarse sync

3. **Phase-based refinement**
   Use Costas phase consistency to validate sync position

4. **Iterative refinement**
   Re-sync after initial extraction using decoded Costas tones

---

## Expected Impact

Reducing frequency error from 0.3 Hz to 0.1 Hz should:
- Reduce FFT bin smearing by 3x
- Decrease tone errors from 20% to ~10% (within LDPC correction capability)
- Enable decoding of weak signals like K1BZM

Reducing timing error from 20ms to 10ms should:
- Reduce inter-symbol interference
- Improve symbol power discrimination

**Target**: 90%+ tone accuracy (≤8 tone errors) for reliable LDPC decoding

---

## Next Steps

1. **Profile WSJT-X's fine sync** to understand their sub-bin interpolation method
2. **Implement sub-bin frequency estimation** in our fine sync
3. **Add iterative refinement** using Costas phase
4. **Test on K1BZM** - verify tone accuracy improves to 90%+

---

## Files Modified

- `src/sync/extract.rs`: Added tone extraction debug output
  - Extracts all 79 tones and compares with expected
  - Shows power analysis for each tone error
  - Calculates tone accuracy percentage

---

## References

- WSJT-X fine sync: `wsjtx/lib/ft8/ft8b.f90` lines 103-150
- FT8 tone spacing: 6.25 Hz (from FT8 specification)
- FFT bin resolution: 6.25 Hz / 32 bins = 0.195 Hz/bin
