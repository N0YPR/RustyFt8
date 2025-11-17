# Multi-Pass Decoding Implementation Status

## Session Summary - 2025-11-17

### ✅ Completed

1. **Multi-Pass Infrastructure** - Complete and working
   - `decode_ft8_multipass()` with N-pass capability
   - Signal subtraction between passes
   - Adaptive sync threshold (reduces 20% per pass)
   - Proper deduplication across passes

2. **Fixed Critical Bugs**
   - Tone extraction: Re-encode 91-bit LDPC message to 174-bit codeword before mapping
   - Negative time offset: Handle DT < 0 without integer overflow
   - Time refinement comparison: Fix search logic (was comparing to NEG_INFINITY)

3. **Time Offset Refinement** - Working but insufficient
   - Searches ±60 samples in 15-sample steps
   - Successfully finds better alignments (-60 to +60 samples)
   - Still only achieves -0.4 dB power reduction

### ❌ Blocking Issue: Poor Signal Subtraction

**Test Results:**
```
Synthetic signal (tests/subtract_debug_test.rs):
  Power reduction: -40.3 dB ✅

Real FT8 recording (210703_133430.wav):
  @ 2572.7 Hz: -0.4 dB (w/ +60 sample refinement) ❌
  @ 2853.9 Hz: -0.0 dB (w/ -60 sample refinement) ❌
  @ 2156.8 Hz: -0.2 dB (w/ -45 sample refinement) ❌
  @ 591.4 Hz: -0.0 dB (w/ -60 sample refinement) ❌
  @ 398.9 Hz: -0.0 dB (w/ -60 sample refinement) ❌
```

**Multi-Pass Results:**
- Pass 1: 5 decodes (expected 5-6)
- Pass 2: 0 new decodes (expected 8-12 additional)
- Target: 22 decodes (WSJT-X)

### 🔍 Root Cause Analysis

**Why synthetic works but real doesn't:**

| Factor | Synthetic | Real FT8 |
|--------|-----------|----------|
| Tone accuracy | Exact match | LDPC may have corrected bit errors |
| Signal purity | Single clean signal | Multiple overlapping signals |
| Frequency stability | Perfect | May have drift/doppler |
| Phase | Controlled | Unknown carrier phase |
| Propagation | None | Fading, multipath, noise |

**The Core Problem:**

We reconstruct the signal from **LDPC-corrected tones**, but the actual audio contains the signal with potential bit errors. Example:

```
Transmitted tones:     [3,1,4,0,6,5,2,7,2,5,...]  (what we hear)
Noisy LLRs:            [garbled soft decisions]
LDPC decode:           [3,1,4,0,6,5,2,0,3,7,...]  (corrected 3 bit errors!)
Our reconstruction:    Based on corrected tones
Result:                Phase/amplitude mismatch → poor subtraction
```

**What WSJT-X Does (from subtractft8.f90):**

WSJT-X likely uses:
1. **Hard decisions from demodulator** (before LDPC) for tone reconstruction
2. **Phase tracking** to align carrier phase
3. **Spectral residual metric** to validate subtraction quality
4. **Iterative refinement** to optimize alignment

### 📊 Progress vs. Plan

From `MULTIPASS_ANALYSIS.md` predictions:

| Implementation | Predicted | Actual | Gap |
|----------------|-----------|--------|-----|
| Current (BP only) | 6 | 5 | ✅ Close |
| + Signal subtraction | 14-18 | 5 | ❌ Blocked |
| + OSD | 16-22 | - | - |
| + A Priori | 17-23 | - | - |

**Conclusion:** Multi-pass infrastructure works, but signal subtraction effectiveness blocks progress.

### 🎯 Next Steps (UPDATED 2025-11-17)

**SIGNAL SUBTRACTION BLOCKED** ❌

After extensive investigation (see [SUBTRACTION_INVESTIGATION.md](SUBTRACTION_INVESTIGATION.md)):
- ✅ Implemented re-encoded tones (matches WSJT-X)
- ✅ Implemented frequency refinement (±3 Hz)
- ✅ Implemented time refinement (±90 samples, matches WSJT-X)
- ✅ Implemented spectral residual metric (matches WSJT-X algorithm exactly)

**Result:** Still only **-0.4 dB power reduction** on real signals (vs -40.3 dB on synthetic).

**Root Cause:** Missing real-world signal characteristics:
- Phase tracking/continuity
- Multipath propagation effects
- Signal model accuracy (GFSK parameters, numerical precision)

**Recommended Path Forward:**

**Option A: Validate WSJT-X Behavior** (HIGHEST PRIORITY)
- Run WSJT-X `jt9` on test file with diagnostics
- Check if WSJT-X actually achieves good subtraction or if gap is elsewhere (OSD, A Priori)
- **Impact:** May reveal we're solving the wrong problem
- **Effort:** Low (just run their tool)

**Option B: Phase Tracking**
- Save phase estimates during symbol extraction
- Apply same phase trajectory during reconstruction
- **Impact:** Could be 10-20 dB improvement if phase is the issue
- **Effort:** Moderate

**Option C: Pivot to Decoder Improvements**
- Accept imperfect subtraction, simplify code
- Focus on OSD, A Priori, LLR normalization
- **Impact:** May have bigger payoff than perfect subtraction
- **Effort:** Low to moderate

**Recommendation:** Start with Option A to validate assumptions before further subtraction work.

### 📁 Modified Files

- [src/decoder.rs](../src/decoder.rs): Multi-pass, tone extraction, debug output
- [src/subtract.rs](../src/subtract.rs): Time refinement, power diagnostics
- [src/lib.rs](../src/lib.rs): Export decode_ft8_multipass
- [tests/multipass_test.rs](../tests/multipass_test.rs): Multi-pass validation

### 🔬 Key Implementation Details

**Tone Extraction (decoder.rs:130-143):**
```rust
// LDPC returns 91-bit message, need 174-bit codeword for symbol mapping
let mut codeword = bitvec![u8, Msb0; 0; 174];
ldpc::encode(&decoded_bits, &mut codeword);  // Re-encode

let mut tones = [0u8; 79];
symbol::map(&codeword, &mut tones)?;  // Map to tones
// ⚠️ Problem: These are corrected tones, not original noisy tones!
```

**Time Refinement (subtract.rs:163-200):**
```rust
// Search ±60 samples for minimum power after subtraction
for offset in [-60, -45, -30, -15, 0, 15, 30, 45, 60] {
    let test_time = time_offset + offset / 12000.0;
    let audio_copy = audio.clone();
    subtract_at_offset(&mut audio_copy, tones, freq, test_time);
    if power_after(audio_copy) < best_power {
        best_offset = offset;
    }
}
// ✅ Works: Successfully finds better alignments
// ❌ Insufficient: Still only -0.4 dB reduction
```

### 📚 References

- WSJT-X subtractft8.f90: Signal subtraction algorithm
- WSJT-X ft8b.f90: Demodulation and hard decision logic
- FT8 Protocol Spec: https://wsjt.sourceforge.io/FT4_FT8_QEX.pdf
- [SIGNAL_SUBTRACTION_PLAN.md](SIGNAL_SUBTRACTION_PLAN.md): Original implementation plan
- [MULTIPASS_ANALYSIS.md](MULTIPASS_ANALYSIS.md): Performance gap analysis
