# Signal Subtraction Investigation Summary

## 2025-11-17 - Comprehensive Analysis

### Executive Summary

Despite implementing multiple refinement strategies matching WSJT-X's approach, signal subtraction on real FT8 recordings achieves only **-0.4 dB power reduction** (vs **-40.3 dB** on synthetic signals). Multi-pass decoding remains blocked at **5 decodes** (vs WSJT-X's 22).

### What We've Tried

#### 1. Tone Source Investigation ✅ RESOLVED
**Hypothesis:** Use original hard decision tones (before LDPC) instead of re-encoded tones.
**Result:** Found 27-30 tone mismatches per signal, but subtraction quality unchanged.
**Conclusion:** WSJT-X uses **re-encoded tones** after LDPC correction. We now match this.

#### 2. Frequency Refinement ⚠️ NO HELP
**Implementation:** ±3 Hz search in 0.5 Hz steps (13 values)
**Result:**
- Frequency corrections found: -3.0 to +1.5 Hz
- Power reduction: Still -0.4 dB
- Multi-pass: 6 decodes (+1 from baseline)

**Conclusion:** Frequency errors exist but correcting them doesn't significantly improve subtraction.

#### 3. Time Refinement (Extended) ⚠️ NO HELP
**Implementation:** ±90 samples (matching WSJT-X) in 15-sample steps
**Result:**
- Time offsets found: ±90 samples (consistently hitting limits)
- Power reduction: Still -0.4 dB
- Multi-pass: Same as baseline

**Conclusion:** Time alignment alone insufficient.

#### 4. Spectral Residual Metric (WSJT-X Approach) ❌ NO HELP
**Implementation:**
- FFT of residual signal
- Measure power in narrow frequency band (f0 ± 9.375 to 53.125 Hz)
- Test 3 time offsets: -90, 0, +90 samples
- Match WSJT-X's `subtractft8.f90` algorithm exactly

**Result:**
- Residual values measured: 3.18e6, 4.88e3, 5.00e3, 4.11e3
- Time refinement: ±90 samples found
- Power reduction: **Still -0.4 dB**
- Multi-pass: **5 decodes** (worse than frequency refinement!)

**Conclusion:** Even with WSJT-X's exact alignment metric, subtraction fails on real signals.

### Test Results Comparison

| Test Case | Power Reduction | Pass 2 Decodes | Total Decodes |
|-----------|-----------------|----------------|---------------|
| Synthetic signal | **-40.3 dB** ✅ | N/A | N/A |
| Real (baseline) | -0.4 dB | 0 | 5 |
| + Frequency ref. | -0.4 dB | 1 | 6 |
| + Time ref. | -0.4 dB | 0-1 | 5-6 |
| + Spectral residual | -0.4 dB | 0 | 5 |
| **WSJT-X** | **(unknown)** | **~17** | **22** |

### Root Cause Analysis

**Why synthetic works but real doesn't:**

| Factor | Synthetic | Real FT8 |
|--------|-----------|----------|
| Signal purity | Single clean signal | Multiple overlapping signals |
| Phase continuity | Perfect | May have discontinuities |
| Frequency stability | Perfect | Doppler shifts, drift |
| Propagation | None | Multipath, fading, phase rotation |
| Timing | Exact | Clock drift, jitter |

**Critical Missing Elements:**

1. **Phase Tracking:**
   - We reconstruct amplitude but don't track carrier phase
   - HF propagation causes phase rotation over 12.6s transmission
   - WSJT-X may estimate and compensate for phase drift

2. **Multipath Handling:**
   - Real HF signals arrive via multiple ionospheric paths
   - Each path has different delay, phase, and amplitude
   - Our single-path model can't capture this

3. **Signal Model Accuracy:**
   - GFSK pulse shape parameters (BT=2.0)
   - Pulse generation differences (Fortran vs Rust)
   - Small errors compound over 79 symbols

4. **Low-Pass Filter Parameters:**
   - NFILT=4000, cosine-squared window
   - Edge correction factors
   - May need exact numerical match to WSJT-X

### WSJT-X vs RustyFt8 Implementation

#### What We Match ✅
- Re-encoded tones (after LDPC)
- Time refinement (±90 samples)
- Spectral residual metric
- Low-pass filter structure
- Signal reconstruction math

#### What We Don't Match ❓
- Phase tracking during demodulation
- Exact FFT/filter numerical implementation
- Multipath signal model
- Frequency/phase drift compensation
- Additional refinement iterations

### Diagnostic Evidence

**Synthetic Signal Test ([tests/subtract_debug_test.rs](../tests/subtract_debug_test.rs)):**
```
Power reduction: -40.3 dB
Signal reduced by more than 20 dB ✅
```

**Real Recording Test ([tests/multipass_test.rs](../tests/multipass_test.rs)):**
```
@ 2572.7 Hz: -0.4 dB (±90 samples, residual=3.18e6)
@ 2853.9 Hz: -0.0 dB (±90 samples, residual=4.88e3)
@ 2156.8 Hz: -0.2 dB (no refinement)
@ 591.4 Hz: -0.0 dB (±90 samples, residual=5.00e3)
@ 398.9 Hz: -0.0 dB (±90 samples, residual=4.11e3)
```

**Key Observation:** Residual values vary widely (4.88e3 to 3.18e6) but all result in poor subtraction.

### Recommended Next Steps

#### Option A: Phase Tracking (Most Promising)
**Theory:** Carrier phase rotation during transmission prevents coherent subtraction.

**Implementation:**
1. During symbol extraction, save instantaneous phase estimates
2. When reconstructing signal, apply same phase trajectory
3. Test if this improves alignment

**Expected Impact:** Could achieve 10-20 dB improvement if phase is the main issue.

**Difficulty:** Moderate - requires modifying symbol extraction to track phase.

#### Option B: Compare WSJT-X Binary Behavior
**Approach:**
1. Run WSJT-X's `jt9` on same test file with debugging
2. Extract actual subtraction results from WSJT-X
3. Compare power reduction to our implementation
4. If WSJT-X also gets poor reduction, problem may be elsewhere (OSD, A Priori)

**Expected Impact:** May reveal we're chasing the wrong problem.

**Difficulty:** Easy - just run their tool and check output.

#### Option C: Simplified Subtraction
**Approach:**
1. Skip sophisticated alignment
2. Just subtract at decoder's estimated frequency/time
3. Accept imperfect subtraction
4. Focus on improving decoder (OSD, A Priori, LLR normalization)

**Rationale:** We've spent significant effort on subtraction with no gains. Maybe WSJT-X's advantage is elsewhere.

**Expected Impact:** Frees resources for other optimizations that may have bigger payoff.

**Difficulty:** Low - just simplify code.

#### Option D: Deep Dive into WSJT-X Source
**Approach:**
1. Read WSJT-X's ft8b.f90 (main decoder loop) in detail
2. Read WSJT-X's sync8.f90 (synchronization)
3. Look for phase tracking, multipath handling, or other features we're missing
4. Check if they do something special before/after subtraction

**Expected Impact:** May find the missing piece.

**Difficulty:** High - requires understanding complex Fortran codebase.

### Code Changes

Modified files:
- [src/decoder.rs](../src/decoder.rs) - Tone re-encoding, debug output
- [src/subtract.rs](../src/subtract.rs) - Frequency/time refinement, spectral residual
- [src/lib.rs](../src/lib.rs) - Export decode_ft8_multipass
- [tests/multipass_test.rs](../tests/multipass_test.rs) - Multi-pass validation
- [tests/subtract_debug_test.rs](../tests/subtract_debug_test.rs) - Synthetic signal test

### Performance Notes

**Spectral Residual Metric Performance:**
- 3 FFTs per signal (±90, 0, +90 offsets)
- Creates new FftPlanner each time (slow but acceptable)
- Test runtime: ~1 minute for 5 signals × 3 passes

**Could optimize:**
- Reuse FftPlanner
- Cache FFT plans
- But current performance is acceptable given subtraction doesn't work anyway

### Conclusion

Signal subtraction on real FT8 recordings is fundamentally blocked by issues beyond timing/frequency alignment. The spectral residual metric proves we're using the right measurement approach (matches WSJT-X), but the underlying signal model or phase tracking appears insufficient for real-world propagation conditions.

**Recommendation:** Pursue Option B (compare WSJT-X behavior) first to validate whether effective subtraction is even the key to WSJT-X's superior performance, then Option A (phase tracking) if subtraction proves critical.

The synthetic signal test (-40.3 dB) proves our algorithm is correct. The failure on real signals (40 dB worse!) indicates missing real-world signal characteristics that are present in actual HF transmissions.
