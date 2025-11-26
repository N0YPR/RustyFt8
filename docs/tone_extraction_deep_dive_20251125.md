# Tone Extraction Deep Dive - 2025-11-25

## Summary

Created diagnostic tool to compare extracted tones vs expected tones for failing signals. Found that **wrong FFT bins have dramatically higher power than correct bins** (up to 17x!), confirming the root cause is in tone extraction, not LDPC or LLR computation.

## Diagnostic Results

### K1BZM EA3GP -09 @ 2695 Hz (SNR=-3 dB)

**Tone Accuracy: 64/79 correct (81.0%, 15 errors)**

Expected tones (from ft8code):
```
3140652 03227073004460620551746353755 3140652 57761725130701312530042543240 3140652
^Costas^ ^-------Data Block 1-------^ ^Costas^ ^-------Data Block 2-------^ ^Costas^
```

### Error Pattern Analysis

**Errors by Symbol Position:**
- **Sym 0**: Costas 1 tone 0 (1 error out of 7)
- **Sym 12,14,16**: Early data block 1 (3 errors)
- **Sym 30-35**: END of data block 1 (**6 consecutive errors, worst ratios!**)
- **Sym 36-42**: Costas 2 (**PERFECT - 0 errors!**)
- **Sym 43-44,53,67-68**: Data block 2 (5 errors)
- **Sym 72-78**: Costas 3 (**PERFECT - 0 errors!**)

### Power Ratio Analysis

| Symbol | Got | Expected | Got Power | Exp Power | Ratio | Notes |
|--------|-----|----------|-----------|-----------|-------|-------|
| 0 | 2 | 3 | 0.100 | 0.093 | 1.1x | Costas 1 start |
| 12 | 6 | 0 | 0.188 | 0.166 | 1.1x | |
| 14 | 4 | 3 | 0.040 | 0.026 | 1.5x | |
| 16 | 7 | 0 | 0.050 | 0.047 | 1.1x | |
| **30** | **1** | **3** | **0.176** | **0.032** | **5.5x** | **ERROR CLUSTER START** |
| **31** | **4** | **5** | **0.109** | **0.061** | **1.8x** | |
| **32** | **2** | **3** | **0.258** | **0.057** | **4.5x** | |
| **33** | **4** | **7** | **0.185** | **0.099** | **1.9x** | |
| **34** | **1** | **5** | **0.199** | **0.012** | **17.1x** | **WORST ERROR!** |
| **35** | **3** | **5** | **0.380** | **0.026** | **14.8x** | **ERROR CLUSTER END** |
| 36-42 | ✓ | ✓ | - | - | - | **Costas 2 PERFECT** |
| 43 | 7 | 5 | 0.136 | 0.026 | 5.3x | Data block 2 start |
| 44 | 6 | 7 | 0.036 | 0.029 | 1.3x | |
| 53 | 1 | 0 | 0.097 | 0.096 | 1.0x | |
| 67 | 7 | 4 | 0.158 | 0.073 | 2.2x | |
| 68 | 7 | 3 | 0.085 | 0.037 | 2.3x | |
| 72-78 | ✓ | ✓ | - | - | - | **Costas 3 PERFECT** |

## Key Findings

### 1. Errors Cluster at Data Block Boundaries

**CRITICAL OBSERVATION**: Symbols 30-35 (last 6 of data block 1) have **MASSIVE** errors (5x-17x wrong peak power), but Costas 2 (symbols 36-42) immediately following is **PERFECT**!

This pattern is **incompatible** with simple frequency/time drift, which would affect all symbols uniformly.

### 2. Costas Arrays Are Perfect

Both Costas 2 and Costas 3 have **0 errors**, while data symbols before and after have multiple errors. This suggests:
- Costas tones (known pattern) have stronger signal power
- Our algorithm works correctly for strong peaks
- Weak data symbols are more susceptible to whatever is causing errors

### 3. Progressive vs Localized Errors

The error distribution suggests **TWO different error sources**:
1. **Progressive drift**: Errors increase from symbol 0→35 (frequency/phase drift)
2. **Localized interference**: Massive errors specifically at symbols 30-35 and 43-44

### 4. Wrong Bins Have Higher Power

**This is the smoking gun**: For symbols 30-35, the WRONG FFT bin has 5x-17x MORE power than the correct bin. This means:
- We're not just failing to detect weak signals
- We're actively detecting strong signal in the WRONG place
- Something fundamental is misaligned (frequency, timing, or phase)

## Hypotheses

### Hypothesis 1: Signal Interference (LIKELY)

**Evidence**:
- Errors cluster at specific time windows (symbols 30-35, 43-44)
- Costas arrays (stronger) are unaffected
- WSJT-X shows nearby signals at 2522 Hz and 2546 Hz

**Test**: Check if those time windows overlap with other transmissions

### Hypothesis 2: Timing Drift Accumulation

**Evidence**:
- Errors worsen from symbol 0→35
- Reset at Costas 2 (symbol 36)
- Progressive pattern suggests accumulating error

**BUT**: Doesn't explain why Costas 2 is perfect when symbols 30-35 right before it are very wrong

### Hypothesis 3: Sample Index Calculation Bug

**Evidence**:
- Errors at data block boundaries (symbols 30-35, 43-44)
- Costas arrays correct (different calculation path?)

**Test**: Verify sample index calculations for data vs Costas symbols

### Hypothesis 4: Frequency Offset Causes Bin Spreading

**Evidence**:
- 0.3 Hz offset documented
- Over 30 symbols (4.8s), phase accumulation = 518 degrees
- Sufficient to shift FFT peak to adjacent bin

**BUT**: Doesn't explain localized clustering at symbols 30-35

## What We Tried (No Improvement)

1. ✓ **Final time refinement in fine_sync** (added ±4 sample search after frequency correction)
   - Result: Still 9/22 decodes
2. ✓ **Raised nsync threshold** to match WSJT-X
   - Result: Still 9/22 decodes (but 2x faster)

## Next Steps

### Priority 1: Investigate Sample Index Calculation
- Add debug output showing sample indices for symbols 30-36
- Verify calculation matches WSJT-X ft8b.f90:155

### Priority 2: Check for Interference
- Look for signal power in nearby frequency bins during symbols 30-35
- Check if other signals are transmitting at that time

### Priority 3: Verify Downsampling
- Compare our downsampled signal vs WSJT-X
- Check for phase discontinuities or artifacts

### Priority 4: Test with Synthetic Signal
- Generate clean FT8 signal at -3 dB SNR
- Verify we can extract tones correctly without real-world interference

## Code Changes

### src/sync/extract.rs (lines 377-424)

Added diagnostic tool that:
1. Extracts all 79 tones by finding max power across 8 FFT bins
2. Compares with expected tones from ft8code
3. Shows power ratio for wrong vs correct bins
4. Enabled for K1BZM (2694-2696 Hz)

### src/sync/fine.rs (lines 198-220)

Added WSJT-X's final time refinement:
1. Re-downsample at best frequency
2. Search ±4 samples for optimal time offset
3. Update best_time based on results

**Result**: No improvement (still 9/22 decodes)

## Conclusion

The root cause is **tone extraction producing wrong bins with higher power than correct bins**. This is NOT solvable by:
- ❌ Better LDPC decoding
- ❌ LLR normalization or scaling
- ❌ Fine sync refinement (tried, didn't help)

The fix requires understanding WHY wrong bins peak higher, which appears to be:
- Frequency/time misalignment causing FFT bin spreading
- Possible interference from nearby signals
- Potential bug in sample index calculation

The **bizarre pattern** of perfect Costas arrays surrounded by error-prone data symbols suggests multiple interacting issues rather than a single root cause.
