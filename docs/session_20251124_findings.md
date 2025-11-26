# Investigation Session 2025-11-24 - Findings

## Current Status: 9/22 Messages (41%)

### What We Implemented

1. **Normalized LLR (WSJT-X Pass 4)** ✓
   - Result: No improvement
   - K1BZM failed even with mean_abs_LLR=13.35 (5x amplification)

2. **Raised nsync Threshold** ✓
   - Changed from nsync>3 to nsync>6 (matching WSJT-X)
   - Then temporarily lowered back to nsync>3 for testing
   - Result: No improvement in either case

3. **Added Final Time Refinement in Fine Sync** ✓
   - Implemented WSJT-X's ±4 sample refinement after frequency search
   - Result: No improvement (frequency/timing still off by 0.3 Hz / 20ms)

### Root Cause Identified: Tone Extraction Errors

**K1BZM EA3GP @ 2695 Hz:**
- **Tone Accuracy: 63/79 (79.7%)** ← 20% error rate
- **16 tone errors** = 48 corrupted bits = **28% bit error rate**
- Exceeds LDPC correction capability (~10-15% max)

**Critical Finding**: Wrong tones have HIGHER FFT power than correct tones!

| Symbol | Expected→Got | Exp Power | Got Power | Ratio |
|--------|--------------|-----------|-----------|-------|
| 30 | 3→1 | 0.032 | 0.176 | **5.5x** |
| 34 | 5→1 | 0.012 | 0.199 | **16.6x** |
| 35 | 5→3 | 0.026 | 0.380 | **14.6x** |

**We're not detecting noise - we're detecting real signal in the WRONG FFT bin!**

### Root Cause: Poor Costas Sync Quality

Candidates we find but can't decode:
- 466.2 Hz (N1PJT HB9CQK -10, expected SNR=-2 dB)
- 722.7 Hz (A92EE F5PSR -14, expected SNR=-7 dB)
- 1196.8 Hz (CQ F5RXL IN94, expected SNR=-2 dB)

All are rejected or fail with:
- **nsync=4-6/21** (19-29% Costas accuracy)
- Compare to WSJT-X: nsync=13-20/21 (62-95% accuracy) for same signals

**The fundamental issue is that our symbol extraction produces wrong tones for Costas arrays.**

---

## Why Our Fixes Didn't Help

### 1. Normalized LLR - Doesn't Fix Wrong Bits
Normalized LLR can't fix fundamentally incorrect tone extraction. Amplifying confidence in wrong bits doesn't help.

### 2. Lower nsync Threshold - Still Fails LDPC
Candidates with nsync=4-6 are being extracted and attempted, but they have 20-30% tone error rate, which exceeds LDPC's correction capability.

### 3. Fine Sync Refinement - Still Off by 0.3 Hz
Even with final time refinement matching WSJT-X's algorithm, we're still off by 0.3 Hz / 20ms. This suggests a deeper issue in:
- Downsample implementation
- FFT window alignment
- Symbol timing computation

---

## The Real Problem: Symbol Extraction Pipeline

The issue is NOT in:
- ✅ LDPC decoder (works fine when given good bits)
- ✅ LLR normalization (correct implementation)
- ✅ Fine sync algorithm (matches WSJT-X's approach)

The issue IS in:
- ❌ **Symbol extraction**: Producing wrong tones (79.7% accuracy for K1BZM)
- ❌ **Costas sync**: Getting 4-6/21 instead of 13-20/21
- ❌ **FFT bin accuracy**: Wrong bins have higher power than correct bins

### Evidence

For K1BZM, the symbol extraction produces:
```
Extracted: 2140652 03227674074460620551746142413 ...
Expected:  3140652 03227073004460620551746353755 ...
```

Even the **first Costas sync tone is wrong** (2 instead of 3), and this cascades through data extraction.

---

## Hypothesis: FFT Windowing or Timing Issue

When we extract each symbol with FFT:
1. We take 32 samples per symbol (at 200 Hz downsampled rate)
2. FFT produces 8 bins (one per tone, 6.25 Hz spacing)
3. We pick the bin with maximum power

**Problem**: For weak signals with 0.3 Hz frequency offset:
- Energy spreads across multiple FFT bins
- Wrong bin can have higher power than correct bin (seen in data!)
- This produces systematically wrong tones

### WSJT-X vs Our Implementation

**WSJT-X** (ft8b.f90 lines 154-161):
```fortran
do k=1,NN
  i1=ibest+(k-1)*32
  csymb=cmplx(0.0,0.0)
  if( i1.ge.0 .and. i1+31 .le. NP2-1 ) csymb=cd0(i1:i1+31)
  call four2a(csymb,32,1,-1,1)
  cs(0:7,k)=csymb(1:8)/1e3
  s8(0:7,k)=abs(csymb(1:8))
enddo
```

**Our Implementation** (extract.rs lines 294-330):
```rust
for k in 0..79 {
    let start = (best_offset + (k as i32) * nsps_down as i32) as usize;
    let end = start + nsps_down;

    // FFT on this symbol
    let mut fft_input = vec![Complex::zero(); nsps_down];
    for i in 0..nsps_down.min(cd.len() - start) {
        fft_input[i] = Complex::new(cd[start + i].0, cd[start + i].1);
    }

    let mut fft_output = vec![Complex::zero(); nsps_down];
    fft_planner.plan_fft_forward(nsps_down).process(&mut fft_input, &mut fft_output);

    // Extract 8 tones
    for tone in 0..8 {
        cs[tone][k] = (fft_output[tone].re, fft_output[tone].im);
        s8[tone][k] = fft_output[tone].norm();
    }
}
```

**Both look equivalent**, but there may be subtle differences in:
- FFT normalization (WSJT-X divides by 1e3)
- Index calculations
- Boundary handling

---

## Critical Discovery: Coarse Sync Spurious Peaks

**Date**: 2025-11-24 (continuation)

### Spurious Peak Analysis for 466 Hz Signal

Added debug output to coarse sync (sync2d correlation). For 465.8 Hz bin:

```
lag=-2 (dt=-0.10s, WSJT-X): sync=0.561  ← True peak
lag=7  (dt=0.26s, ours):     sync=13.220 ← Spurious peak (23x stronger!)
```

**Pattern**: Exponential ramp from lag=1 to lag=7 (0.655 → 13.220) suggests systematic alignment issue.

### WSJT-X Two-Stage Peak Finding

**Key Finding**: WSJT-X `sync8.f90` lines 92-97 uses **two separate searches**:

1. **Restricted search** `[-10, +10]` (±0.4s): Finds nearby peaks, avoids distant spurious peaks
2. **Full search** `[-62, +62]` (±2.5s): Finds any strong peak
3. **Normalizes separately**: `red` and `red2` normalized by their own 40th percentiles (lines 110, 116)
4. **Adds both as candidates**: Lines 120-133 add both if different

**Why This Matters**: For 466 Hz:
- Restricted search finds: lag=-2, sync=0.561 (true signal!)
- Full search finds: lag=7, sync=13.220 (spurious peak)
- WSJT-X tries to decode **both**, success comes from lag=-2

**Our Implementation**: Single full-range search → picks lag=7 (strongest) → wrong time → fails

### Implementation Attempt

Implemented two-stage peak finding with separate normalization in `src/sync/candidate.rs`:
- Result: **8/22 decodes** (worse than original 9/22)
- Issue: Need to understand why adding more candidates hurts performance
- Possible causes:
  1. Decoder time limits - too many candidates to process
  2. Duplicate candidate handling needs improvement
  3. Candidate ranking after merge needs WSJT-X's approach

## Latest Discovery: Two-Stage Approach Fails Due to Spurious Peak Location

**Date**: 2025-11-24 (continued)

### Critical Finding

The two-stage peak finding doesn't help because **the spurious peak is within the restricted range**:

```
NEAR search [-10, +10]: finds lag=7, sync=13.220  (spurious!)
FAR search [-62, +62]: finds lag=7, sync=13.220   (same!)
→ Condition `best_lag_far != best_lag_near` fails → NO second candidate added
```

**True peak at lag=-2 (sync=0.561) never wins** because:
- It's 23x weaker than lag=7
- Both are within [-10, +10] range
- Maximum sync wins in both searches

### Why WSJT-X Succeeds

Either:
1. **Their sync2d values differ** - spurious peak not as strong
2. **Additional preference logic** - favors dt≈0 over distant peaks
3. **Our sync2d computation bug** - creating artificial spurious peaks

### Evidence from Symbol Extraction

For 466 Hz at dt=0.26s (wrong time):
- Symbols 0-1: Correct (lucky alignment)
- Symbols 2-6: **Wrong bins 2-9.5x stronger than correct bins**
- The 360ms offset (2.25 symbols) causes progressive misalignment

## ROOT CAUSE DISCOVERED: Cross-Correlation with Nearby Signals

**Date**: 2025-11-24 (final discovery)

### The Spurious Peak is NOT a Bug - It's Cross-Correlation!

**Evidence from spectrum analysis:**

**lag=-2 (True signal at dt=-0.10s, time m3=298):**
- Spectrum baseline at 466 Hz: 0.000000, 0.000002, 0.000010 (very low)
- Costas3[0] at freq+3: power=0.000020
- Costas3[1] at freq+1: power=0.000009
- Costas3[2] at freq+4: power=0.000015
- Total tc=0.000076

**lag=7 (Spurious peak at dt=0.26s, time m3=307):**
- Spectrum baseline at 466 Hz: 0.000000, 0.000000, 0.000000 (ALL ZERO!)
- Costas3[0] at freq+3: power=0.000173 (8.6x stronger)
- Costas3[1] at freq+1: power=0.000142 (15.8x stronger)
- Costas3[2] at freq+4: power=0.000079 (5.3x stronger)
- Total tc=0.000609 (8x stronger!)

**The baseline spectrum at 466 Hz is ZERO at lag=7**, but the Costas-offset frequency bins (freq+1, freq+3, freq+4) have REAL SIGNAL ENERGY!

### What This Means

The spurious peak at lag=7 (dt=0.26s) is caused by **Costas3 correlating against a different signal** at:
- Time: m3=307-315 (12.3s into recording, 360ms later than true signal)
- Frequency: Offset from 466 Hz by Costas pattern tones (roughly 6.25-25 Hz away)

The Costas pattern's frequency offsets accidentally match another FT8 signal or noise burst at that time/frequency!

### Why This Creates Such Strong Correlation

Costas correlation formula:
```
tc = sum of spectrum[freq + tone_offset][time]
```

At lag=7, Costas3 reads spectrum at m3=307-315, and those time indices contain REAL signal energy in the offset frequency bins. This creates legitimate (but incorrect) Costas correlation.

The baseline calculation then makes it worse:
```
t0 = (t0_raw - t) / 6.0
```

When `t` is large (0.001072), this produces tiny baseline (0.000113), inflating sync to 9.456.

### The Real Question

**Why doesn't WSJT-X have this problem?**

Investigation revealed:
1. **Baseline normalization is identical** - WSJT-X uses the same `t0=(t0-t)/6.0` formula
2. **Two-stage approach is identical** - Both find only ONE candidate (at lag=7, not lag=-2)
3. **WSJT-X likely uses multi-pass subtraction** - Decodes/subtracts interfering signal first, removing the cross-correlation source

### Balance Penalty Experiment

**Attempt**: Penalize sync values when Costas arrays are imbalanced (one dominates).

**Results**:
- Aggressive penalty (ratio²): Reduced spurious peak but also killed real signals (2/22 decodes)
- Gentle penalty (ratio>3): Minor improvement, still missed weak signals (8/22 decodes)
- No penalty (current): 9/22 decodes

**Conclusion**: Balance penalty alone isn't enough. Real signals can have ratio=2-3x due to noise/fading. Spurious peak (ratio=4.8x) needs 3x reduction to be competitive, but this also affects real signals.

## Next Steps (Priority Order)

### Priority 1: Fix Two-Stage Peak Finding Implementation

**Objective**: Correctly implement WSJT-X's two-stage approach without regressing.

**Issues to resolve**:
1. Check if we're hitting max_candidates limit and dropping good candidates
2. Verify candidate deduplication works correctly with doubled candidate count
3. Compare candidate ordering/priority with WSJT-X (do they prefer near peaks?)
4. Check if decoder has time/resource limits being exceeded

**Expected outcome**: Match or exceed 9/22 decodes by finding true peaks.

### Priority 2: Debug Symbol Extraction for Costas Arrays

**Objective**: Even with correct timing, understand why Costas sync is 4-6/21 instead of 13-20/21 for some signals.

**Approach**:
1. Add debug output for Costas array extraction
2. Compare FFT bin powers for all 7 Costas tones
3. Check if the issue is:
   - Wrong FFT bin selection (peak in wrong bin)
   - Timing offset (extracting at wrong sample position)
   - Frequency offset (signal not centered in baseband)

**Expected outcome**: Identify specific bug in symbol extraction.

### Priority 2: Validate Downsample Implementation

**Objective**: Ensure downsampling correctly centers signal at baseband.

**Tasks**:
1. Compare our downsample_200hz vs WSJT-X's ft8_downsample
2. Verify mixing frequency calculation
3. Check filter parameters
4. Test with synthetic signal at known frequency

**Expected outcome**: Fix frequency offset (currently 0.3 Hz off).

### Priority 3: Compare FFT Implementation

**Objective**: Verify our FFT matches WSJT-X's four2a.

**Tasks**:
1. Test on known input (e.g., pure tone)
2. Compare output bin ordering and normalization
3. Check if we need different FFT normalization

**Expected outcome**: Correct any FFT differences.

---

## Why This Is Hard

WSJT-X achieves 22/22 decodes with:
- 95%+ tone accuracy (nsync=19-21/21 for strong signals)
- 80%+ tone accuracy even for weak signals (nsync=13-16/21)

We achieve:
- 95%+ tone accuracy for our 9 successful decodes (nsync=19-21/21)
- **20-30% tone accuracy for failed decodes** (nsync=4-6/21)

The performance cliff is dramatic - signals either decode perfectly or fail completely. There's no graceful degradation.

This suggests a **systematic error** in our symbol extraction that affects weak signals disproportionately.

---

## Files Modified Today

### src/sync/fine.rs
- Added final time refinement (±4 samples) after frequency search
- Matches WSJT-X ft8b.f90:144-152

### src/sync/extract.rs
- Implemented normalized LLR computation
- Added tone extraction debug for K1BZM
- Temporarily lowered nsync threshold (nsync>3) for testing
- Added power analysis for tone errors

### src/decoder.rs
- Added dual-method LLR loop (Standard + Normalized)

### docs/
- tone_extraction_root_cause.md: Comprehensive root cause analysis
- session_20251124_findings.md: This document

---

## Test Data

**Recording**: tests/test_data/210703_133430.wav
- WSJT-X: 22 decodes
- RustyFt8: 9 decodes
- Gap: 13 missing signals

**Example failures** (all found as candidates but fail to decode):
- N1PJT HB9CQK -10 @ 466 Hz, SNR=-2 dB (strong!)
- CQ F5RXL IN94 @ 1197 Hz, SNR=-2 dB (strong!)
- A92EE F5PSR -14 @ 723 Hz, SNR=-7 dB
- K1BZM EA3GP -09 @ 2695 Hz, SNR=-3 dB

All have nsync=4-6/21 and fail LDPC.
