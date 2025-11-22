# Decoder Performance Analysis: Real FT8 Recording

## Test: 210703_133430.wav

### Executive Summary

RustyFt8 decodes **8 of 19** expected messages from the real FT8 recording (42% success rate), while WSJT-X decodes all 22 signals (including 3 at -20 to -24 dB that are not required).

### Performance Comparison

| Metric | RustyFt8 | WSJT-X | Gap |
|--------|----------|--------|-----|
| Total decoded | 8 | 22 | -14 |
| Expected (≥-17 dB) | 8/19 (42%) | 19/19 (100%) | -58% |
| Strong (-3 to 16 dB) | 3/10 (30%) | 10/10 (100%) | -70% |
| Weak (-17 to -9 dB) | 5/9 (56%) | 9/9 (100%) | -44% |

### Root Cause Analysis

#### 1. Coarse Sync Time Offset Errors ⚠️ CRITICAL

**Problem**: Coarse sync finds candidates but assigns **incorrect time offsets** up to 2.3 seconds off.

**Evidence**:
- `WA2FZW DL5AXX RR73`: Found at dt=2.18s (WSJT-X: -0.1s) → **2.28s error**
- `W1DIG SV9CVY -14`: Found at dt=1.62s (WSJT-X: 0.4s) → **1.22s error**
- `KD2UGC F6GCP R-23`: Found at dt=1.70s (WSJT-X: 0.4s) → **1.30s error**
- `XE2X HA2NP RR73`: Found at dt=1.42s (WSJT-X: 0.2s) → **1.22s error**

**Root cause**: `compute_sync2d()` correlation finds **spurious peaks** at wrong time lags, likely due to:
- Multiple overlapping signals creating cross-correlation artifacts
- Noise creating false peaks
- Sync metric (signal/baseline) not robust enough for multi-signal scenarios

**Impact**: Fine sync can only correct ±20ms, so signals with >20ms timing errors fail during extraction.

#### 2. Fine Sync Limited Correction Range

**Problem**: Fine sync searches only **±20ms** (±4 steps of 5ms) around coarse sync estimate.

**Code**: `src/sync/fine.rs:151` - `for dt in -4..=4`

**Impact**: Cannot correct the 1-2 second timing errors from coarse sync.

**Why this matters**: Signal extraction requires precise timing (within ~50ms). With 2.3s errors, extracted symbols are from the wrong time window entirely, causing LDPC decode to fail.

#### 3. SNR Calibration Error 📊

**Problem**: RustyFt8 reports SNR **2-8 dB higher** than WSJT-X.

**Evidence** (8 decoded signals):

| Signal | RustyFt8 SNR | WSJT-X SNR | Delta |
|--------|--------------|------------|-------|
| WM3PEN EA6VQ -09 | 18 dB | 12 dB | +6 dB |
| K1JT HA0DU KN07 | -6 dB | -14 dB | +8 dB |
| W0RSJ EA3BMU RR73 | -8 dB | -16 dB | +8 dB |
| K1JT EA3AGB -15 | -9 dB | -16 dB | +7 dB |
| N1API HA6FQ -23 | -8 dB | -14 dB | +6 dB |
| N1JFU EA6EE R-07 | -7 dB | -12 dB | +5 dB |

**Average discrepancy**: +6.4 dB

**Root cause**: Likely incorrect noise floor estimation or signal power calculation in `src/sync/extract.rs` SNR calculation.

**Impact**:
- SNR filter may reject signals incorrectly
- Cannot trust SNR for quality assessment
- Misleading performance metrics

### Detailed Signal Analysis

#### ✓ Successfully Decoded (8/19)

| Message | Freq | RustyFt8 SNR | WSJT-X SNR | Status |
|---------|------|--------------|------------|--------|
| W1FC F5BZB -08 | 2572 Hz | 17 dB | 16 dB | ✓ Strong signal |
| WM3PEN EA6VQ -09 | 2157 Hz | 18 dB | 12 dB | ✓ Strong signal |
| N1JFU EA6EE R-07 | 642 Hz | -7 dB | -12 dB | ✓ Moderate |
| K1JT HA0DU KN07 | 590 Hz | -6 dB | -14 dB | ✓ Weak |
| W0RSJ EA3BMU RR73 | 400 Hz | -8 dB | -16 dB | ✓ Weak |
| K1JT EA3AGB -15 | 1648 Hz | -9 dB | -16 dB | ✓ Weak |
| XE2X HA2NP RR73 | 2855 Hz | -9 dB | -11 dB | ✓ Weak |
| N1API HA6FQ -23 | 2238 Hz | -8 dB | -14 dB | ✓ Weak |

#### ✗ Missing Strong Signals (3/19) 🚨

These should be trivial to decode:

| Message | Freq | WSJT-X SNR | Candidate Found | Issue |
|---------|------|------------|-----------------|-------|
| CQ F5RXL IN94 | 1197 Hz | -2 dB | ✓ 1195 Hz, sync=3.16, dt=-0.74s | Time offset wrong |
| N1PJT HB9CQK -10 | 466 Hz | -2 dB | ✓ 469 Hz, sync=3.23, dt=0.26s | Possibly false peak |
| K1BZM EA3GP -09 | 2695 Hz | -3 dB | ✓ 2695 Hz, sync=4.89, dt=-0.14s | Unknown decode failure |

#### ✗ Missing Medium Signals (4/19)

| Message | Freq | WSJT-X SNR | Candidate Found | Issue |
|---------|------|------------|-----------------|-------|
| KD2UGC F6GCP R-23 | 472 Hz | -6 dB | ✓ 472 Hz, sync=2.64, dt=1.70s | Time offset 1.3s off |
| A92EE F5PSR -14 | 723 Hz | -7 dB | ✓ 727 Hz, sync=5.99, dt=0.14s | Unknown decode failure |
| W1DIG SV9CVY -14 | 2733 Hz | -7 dB | ✓ 2731 Hz, sync=2.29, dt=1.62s | Time offset 1.2s off |
| K1BZM EA3CJ JN01 | 2522 Hz | -7 dB | ✓ 2525 Hz, sync=2.15, dt=0.22s | Unknown decode failure |

#### ✗ Missing Weak Signal (1/19)

| Message | Freq | WSJT-X SNR | Candidate Found | Issue |
|---------|------|------------|-----------------|-------|
| WA2FZW DL5AXX RR73 | 2546 Hz | -9 dB | ✓ 2543 Hz, sync=38.15, dt=2.18s | Time offset 2.3s off! |

#### ✗ Missing Very Weak Signals (3/19)

| Message | Freq | WSJT-X SNR | Candidate Found | Issue |
|---------|------|------------|-----------------|-------|
| N1API F2VX 73 | 1513 Hz | -17 dB | ✓ 1512 Hz, sync=2.65, dt=0.66s | Time offset questionable |
| CQ DX DL8YHR JO41 | 2606 Hz | -17 dB | ✓ 2607 Hz, sync=3.83, dt=0.46s | Possibly correct, decode failure |
| CQ EA2BFM IN83 | 2280 Hz | -17 dB | ✗ NO CANDIDATE | Not detected at all |

### Key Observations

1. **Candidates ARE being found** for 18/19 expected signals (95%)
2. **Time offset errors** are the primary failure mode (at least 7/11 missed signals)
3. **Strong signals are failing** due to timing errors, not weak signal handling
4. **Even -17 dB signals** have candidates found, but decode fails
5. **One signal** (CQ EA2BFM IN83 at 2280 Hz, -17 dB) has no candidate at all

### Frequency Distribution

**Successfully decoded**: Spread across 400-2855 Hz (no obvious frequency bias)

**Missed signals**: Also spread across 466-2733 Hz (no frequency clustering)

**Conclusion**: Problem is not frequency-dependent.

### Recommended Fixes (Priority Order)

#### 1. Fix Coarse Sync Time Lag Detection 🔴 HIGH PRIORITY

**Goal**: Prevent spurious sync peaks at wrong time lags.

**Approach A - Enhanced Peak Selection**:
- When multiple peaks found per frequency, prefer peak with time offset closest to 0s
- Add penalty term to sync metric for extreme time offsets
- Improve duplicate detection to merge candidates at same frequency

**Approach B - Correlation Robustness**:
- Investigate WSJT-X sync metric calculation
- Consider using only 2 Costas arrays (skip first if starting late)
- Add time-lag consistency check across frequency bins

**Approach C - Multi-hypothesis Testing**:
- Keep multiple time-lag candidates per frequency
- Let LDPC decode decide which is correct
- Requires increasing decode_top_n significantly

#### 2. Expand Fine Sync Search Range 🟡 MEDIUM PRIORITY

**Current**: ±4 steps (±20ms)

**Proposed**: ±40 steps (±200ms) or adaptive based on coarse sync confidence

**Rationale**: Can correct moderate timing errors without full re-implementation

**Risk**: Increases compute time ~10x per candidate

#### 3. Fix SNR Calibration 🟢 LOW PRIORITY

**Investigate**:
- `src/sync/extract.rs` SNR calculation
- Compare noise floor estimation method vs WSJT-X
- Verify signal power measurement

**Impact**: Correctness issue, but not blocking decodes

#### 4. Debug Remaining Failures 🟢 LOW PRIORITY

**Signals with correct-ish timing but still failing**:
- K1BZM EA3GP -09 (2695 Hz, dt=-0.14s) - should decode!
- A92EE F5PSR -14 (723 Hz, dt=0.14s) - should decode!
- K1BZM EA3CJ JN01 (2522 Hz, dt=0.22s) - should decode!

**Action**: Add detailed logging during fine_sync and extract_symbols to understand why these fail.

### Testing Plan

1. **Implement Fix #1** (coarse sync timing)
2. **Re-run test** on 210703_133430.wav
3. **Target**: Decode all 19 expected signals (≥-17 dB)
4. **Measure**:
   - Decode success rate
   - SNR calibration accuracy
   - False positive rate
   - Decode time

### References

- **Test file**: `tests/real_ft8_recording.rs:58` - `test_real_ft8_recording_210703_133430`
- **Coarse sync**: `src/sync/candidate.rs:165` - `coarse_sync()`
- **Sync correlation**: `src/sync/spectra.rs:281` - `compute_sync2d()`
- **Fine sync**: `src/sync/fine.rs:132` - `fine_sync()`
- **Candidate finding**: `src/sync/candidate.rs:35` - `find_candidates()`

### WSJT-X Comparison

WSJT-X achieves 100% decode rate on these signals by:
1. More robust sync peak detection (unknown specifics)
2. Better time-lag selection strategy
3. Accurate SNR calibration
4. Possibly multi-pass decoding with signal subtraction

---

**Analysis Date**: 2025-11-22
**Decoder Version**: RustyFt8 v0.1.0 (commit 10e258c)
**WSJT-X Reference**: v2.7.0
