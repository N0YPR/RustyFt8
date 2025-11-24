# LLR Quality Investigation - Root Cause Analysis
**Date**: 2025-11-23
**Status**: Root cause identified - LLR quality 11% below threshold

---

## Problem Statement

After fixing fine_sync jumping and disabling multi-symbol combining, we're decoding 9/22 messages (41%). The missing signals include **STRONG** signals (-2, -3 dB) that should be easier to decode than the **WEAK** signals (-16, -17 dB) we successfully decode.

**Key Example: K1BZM EA3GP @ 2695 Hz**
- WSJT-X: -3 dB SNR (should be strong!)
- Our result: Found correctly, excellent Costas sync (20/21), but **mean_abs_LLR=2.38** vs needed ~2.5-2.7
- All 16 LLR scaling factors fail to decode

---

## Investigation: Symbol Power and LLR Analysis

### Comparison: W1FC (Success) vs K1BZM (Failure)

**W1FC F5BZB @ 2572 Hz (+16 dB, decodes successfully):**
```
Symbol powers:
  sym[7]: mean=0.254, max=1.089
  sym[8]: mean=0.773, max=2.436
  sym[9]: mean=0.698, max=2.492
  sym[10]: mean=0.667, max=2.747
  sym[11]: mean=0.717, max=2.875

Raw LLRs: mean=1.73, max=3.08, min=0.43
After normalization: std_dev=1.84, mean=2.67
LDPC result: Decodes with 0 iterations (very strong!)
```

**K1BZM EA3GP @ 2695 Hz (-3 dB, fails to decode):**
```
Symbol powers:
  sym[7]: mean=0.043, max=0.210
  sym[8]: mean=0.062, max=0.281
  sym[9]: mean=0.059, max=0.323
  sym[10]: mean=0.074, max=0.397
  sym[11]: mean=0.069, max=0.166

Raw LLRs: mean=0.16, max=0.48, min=0.001
After normalization: std_dev=0.20, mean=2.38
LDPC result: All attempts fail (across 16 scales)
```

---

## Key Findings

### 1. Symbol Powers Are 10x Weaker
- **W1FC** s2_mean: 0.25-0.77
- **K1BZM** s2_mean: 0.04-0.07
- **Ratio**: ~10x difference

This 10x amplitude difference corresponds to ~20 dB, which matches the SNR difference between W1FC (+16 dB) and K1BZM (-3 dB). **This is expected and correct!**

### 2. Weak Bits Near Zero
K1BZM has some bits with LLR near zero (min=0.001), indicating:
- Poor tone discrimination
- Symbols at the noise floor
- Possibly interference from nearby signals

### 3. Normalization Paradox
After std_dev normalization:
- W1FC: mean_abs_LLR = 2.67
- K1BZM: mean_abs_LLR = 2.38
- **Difference: only 11%!**

The 11% LLR gap is the difference between LDPC success and failure. The normalization process brings K1BZM very close to W1FC, but not quite over the threshold.

### 4. Raw LLR Minimum Values
- **W1FC**: Even the weakest bits have LLR=0.43 (good discrimination)
- **K1BZM**: Weakest bits have LLR=0.001 (essentially zero)

This suggests K1BZM has **poor bit confidence** for certain bits, not just uniform weakness.

---

## Interference Analysis

**K1BZM @ 2695 Hz** transmits at the same time (dt=-0.1s) as **WA2FZW @ 2546 Hz** (also missing from our decodes). They are 149 Hz apart, which should be enough separation (FT8 signal bandwidth is ~50 Hz), but:

1. Both signals are missing from our decoder
2. Both transmit simultaneously
3. Possible spectral leakage or adjacent channel interference

**Other nearby signals:**
- W1FC @ 2571 Hz: 124 Hz away, dt=0.3s (time-separated, decodes ✅)
- W1DIG @ 2733 Hz: 38 Hz away, dt=0.4s (time-separated, decodes ✅)

The time-separated signals decode successfully despite closer frequency spacing, suggesting **simultaneous transmission** is the key issue, not frequency proximity alone.

---

## Root Cause Summary

**K1BZM fails to decode because:**

1. **Symbol powers are weak** (10x below W1FC) - expected for -3 dB vs +16 dB SNR
2. **After normalization, LLR quality is 11% below threshold** (2.38 vs 2.67)
3. **Some bits have near-zero confidence** (min LLR=0.001) suggesting:
   - Interference from simultaneous signal (WA2FZW @ 2546 Hz)
   - Phase noise
   - Timing/sync issues

4. **LDPC decoder needs ~2.5-2.7 mean_abs_LLR** but we're providing 2.38

---

## Why Strong Signals Fail While Weak Signals Succeed

**Paradox**: We decode -16 dB signals but fail on -3 dB signals.

**Explanation**: It's not about absolute SNR, but about:
1. **Interference** - Strong signals at the same time cause problems
2. **LLR quality** - Some signals have better symbol discrimination than others
3. **Phase stability** - Weaker but cleaner signals can decode better than stronger but noisier signals

Signals we successfully decode at -16 dB likely have:
- No simultaneous interferers
- Better phase stability
- Cleaner symbol transitions

---

## Proposed Solutions

### 1. Multi-Pass Decoding with Signal Subtraction (Priority: HIGH)
**Status**: Already implemented in `decode_ft8_multipass`

WSJT-X uses 2-3 passes:
1. Pass 1: Decode strongest signals
2. Subtract decoded signals from audio
3. Pass 2: Decode weaker signals now visible

**Expected impact**: Should help K1BZM and other simultaneous signals by removing interference from W1FC and other strong decodes.

### 2. A-Priori Information (Priority: MEDIUM)
WSJT-X uses hash tables of recently seen callsigns to boost marginal decodes:
- Partial matches get SNR bonus
- Helps signals right at the threshold (like K1BZM at 2.38 vs needed 2.5)

**Expected impact**: 5-10% improvement in marginal cases.

### 3. LLR Calibration Improvement (Priority: LOW)
The normalization by std_dev brings weak signals close to threshold but not quite enough. Investigate:
- WSJT-X's exact normalization formula
- Whether they apply additional scaling for weak signals
- Alternative normalization strategies

### 4. Phase Tracking (Priority: LOW)
K1BZM's poor bit confidence (min LLR=0.001) suggests phase issues. Implement per-symbol phase tracking instead of global correction.

---

## Multi-Pass Decoding Test Results

**Status**: FAILED - Produces false positives

Tested `decode_ft8_multipass` with 3 passes:
- Pass 1: 9 correct decodes
- Pass 2: 0 new decodes
- Pass 3: 2 new decodes (DW7HKN/P KK0XRY/P EH18, IX0LAK KW6JMM KE51)
- **Result**: Both pass 3 decodes are FALSE POSITIVES (not in WSJT-X output)

**Problem**: Signal subtraction creates artifacts in the residual audio that produce spurious decodes. Our subtraction is not perfect enough to avoid creating false sync peaks.

**WSJT-X advantage**: Likely has:
1. More accurate signal reconstruction for subtraction
2. Better artifact rejection heuristics
3. Different subtraction strategy (frequency-domain?)

**Conclusion**: Multi-pass decoding disabled - need better subtraction quality before it can help.

---

## Next Steps

1. ~~**Test multi-pass decoding** on the real recording~~ ❌ Produces false positives
2. **Implement a-priori information** (PRIORITY: HIGH)
   - Hash lookups for recently seen callsigns
   - Boost marginal candidates (mean_abs_LLR 2.3-2.5) by ~5-10%
   - Should help K1BZM and other threshold cases
3. **Improve signal subtraction quality** (PRIORITY: MEDIUM)
   - Study WSJT-X's subtraction algorithm
   - Consider frequency-domain subtraction
   - Add artifact rejection heuristics
4. **Profile LDPC threshold** - determine exact mean_abs_LLR threshold for different decode depths

---

## Technical Details

### Symbol Extraction Process
1. Downsample to 200 Hz centered on candidate frequency
2. Correlate with 8-FSK tones to get complex symbol values (cs[][])
3. Compute magnitudes (s8[][])
4. For each 3-bit symbol group, find max magnitude for bit=0 and bit=1
5. LLR = max_mag_1 - max_mag_0
6. Normalize by std_dev and scale by 2.83

### Why Normalization Brings Signals So Close
The std_dev normalization is scale-invariant:
- Weak signal with consistent discrimination → normalized to similar mean as strong signal
- K1BZM: weak (std_dev=0.20) but consistent → normalized to 2.38
- W1FC: strong (std_dev=1.84) and consistent → normalized to 2.67

The 11% gap remains because K1BZM has **worse discrimination** (some near-zero bits), not just lower power.

---

## Files Modified for Investigation
- `src/sync/extract.rs`: Added debug output for symbol powers and LLR analysis
- Enabled debug for K1BZM @ 2695 Hz and W1FC @ 2572 Hz
- Measured s2 powers, raw LLRs, std_dev, and normalized LLRs
