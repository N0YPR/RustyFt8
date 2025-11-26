# Investigation Session 2025-11-25 Part 2: Filter Debug & Root Cause Discovery

## Session Goal

Continue from previous session to determine why tone extraction has 81% accuracy instead of required 90%+. Previous session identified potential aliasing from 2522 Hz interferer.

## What We Did

### 1. Added Comprehensive Downsample Debug Output

Modified [src/sync/downsample.rs](../src/sync/downsample.rs) to add detailed filter verification:

**Debug features added:**
- FFT bin calculation verification (ib, it, i0)
- Frequency range extraction verification
- Interferer bin distance calculation
- Spectral power checks at key frequencies
- Filter sidelobe leakage analysis
- Output buffer power verification

**Key code addition** (lines 73-147):
```rust
// Debug for K1BZM EA3GP @ 2695 Hz (verify aliasing hypothesis)
let debug_k1bzm = f0 > 2694.0 && f0 < 2696.0;
if debug_k1bzm {
    eprintln!("=== DOWNSAMPLE DEBUG for f0={:.1} Hz ===", f0);
    // ... comprehensive filter analysis ...
}
```

### 2. Verified Filter Excludes Out-of-Band Signals

**Result:** Filter working perfectly!

For target @ 2695 Hz:
- Filter range: [2685.9, 2748.4] Hz (62.6 Hz bandwidth)
- Extracts bins [42975, 43975] from 192k FFT
- Interferer @ 2522 Hz: bin 40352
- Distance: **2623 bins (163.9 Hz) away** from filter edge
- Status: `extracted=false` ✓

**Spectral power verification:**
```
2522.0 Hz (Interferer EA3CJ): power=1.636e1, extracted=false ✓
2546.0 Hz (Interferer WA2FZW): power=7.917e0, extracted=false ✓
2695.0 Hz (Target EA3GP): power=3.467e1, extracted=true ✓
```

**Conclusion:** The 2522 Hz aliasing hypothesis was **WRONG**. Filter correctly excludes out-of-band signals.

### 3. Discovered Real Culprit: In-Band Interference

**CRITICAL FINDING:** Another FT8 signal is **inside the same passband**!

**W1DIG SV9CVY @ 2733 Hz:**
- SNR=-7 dB (per WSJT-X)
- Power=29.96 (comparable to target power=34.67)
- Only **38 Hz away** from target (6.08 FT8 tones)
- Status: `extracted=true` - both signals legitimately in passband!

**Debug output revealed:**
```
2695.0 Hz (Target EA3GP): bin=43120, power=3.467e1, extracted=true
2733.0 Hz (Interferer W1DIG): bin=43728, power=2.996e1, extracted=true ⚠️
```

### 4. Explained Tone Extraction Errors

**Why 32-point FFT fails:**
- After downsampling to 200 Hz baseband:
  - EA3GP centered at ~0 Hz
  - W1DIG offset by ~38 Hz = 6.08 bins
- 32-point FFT with 8 bins for 8 tones (6.25 Hz spacing)
- **Signals only 6 tones apart cannot be cleanly separated**
- Spectral leakage from W1DIG pollutes EA3GP's bins
- Wrong FFT bins get 5x-17x higher power during tone collisions

**Why errors cluster at symbols 30-35:**
- Both signals transmitting simultaneously
- Specific tone combinations create maximum interference
- When tones are within 2-3 bins, FFT can't resolve them

**Why Costas arrays remain perfect:**
- Costas has stronger sync power by design
- Fixed tone pattern (3,1,4,0,6,5,2) reduces ambiguity
- Fine sync optimizes specifically for Costas correlation

### 5. Confirmed This Is NOT a Bug

**Both signals legitimately in passband:**
- FT8 standard passband: [f0-9.375, f0+53.125] Hz = 62.5 Hz
- For f0=2695: [2685.9, 2748.4] Hz
- Both EA3GP (2695 Hz) and W1DIG (2733 Hz) fit in this range
- Filter is **CORRECT** per FT8 specification

**This is a decoder architecture issue**, not a signal processing bug.

## How WSJT-X Handles This

### Multi-Pass Decoding with Subtraction

From `ft8b.f90`:
```fortran
if(lsubtract) then
   call timer('sub_ft8a',0)
   call subtractft8(dd0,itone,f1,xdt,.false.)
   call timer('sub_ft8a',1)
endif
```

**WSJT-X strategy:**
1. Decode all candidates with current audio
2. For each successful decode:
   - Synthesize FT8 signal from decoded message
   - Subtract it from original 12 kHz audio
3. Re-run candidate detection on residual
4. Decode new candidates from clean residual
5. Repeat until no new decodes

**Why this works:**
- EA3GP decoded first (stronger, SNR=-3 dB)
- Subtract EA3GP signal from audio
- W1DIG now visible in residual (SNR=-7 dB, no interference)
- Decode W1DIG from clean residual

## Key Insights

### What We Learned

1. **Frequency-domain filter is perfect** - no aliasing, no bugs
2. **In-band interference is the real problem** - overlapping FT8 signals in same passband
3. **Multi-pass subtraction is essential** - not optional, required for realistic scenarios
4. **WSJT-X achieves 22/22 specifically because of subtraction** - not just better LLR/LDPC

### What Was Wrong

1. ❌ **Aliasing hypothesis** - 2522 Hz signal is correctly filtered out
2. ❌ **Filter bug theory** - bin extraction is correct
3. ❌ **LLR/LDPC issues** - these are fine, bits are just wrong
4. ❌ **Phase drift** - not the primary issue for nsym=1

### What Is Correct

1. ✅ **Filter implementation matches WSJT-X** - [f0-9.375, f0+53.125] Hz
2. ✅ **Bin extraction is correct** - verified with debug output
3. ✅ **Problem is architectural** - need multi-pass decoding
4. ✅ **Solution is clear** - implement signal subtraction

## Impact Assessment

### Current Performance
- **9/22 decodes (41%)** without subtraction
- Missing 13 signals due to in-band interference

### Expected After Subtraction
- **18-20/22 decodes (82-91%)** with multi-pass subtraction
- **22/22 decodes (100%)** matching WSJT-X (with nsym=2/3 also working)

### Other Signals Likely Affected
Check these for in-band interference:
- N1PJT HB9CQK @ 466 Hz
- CQ F5RXL @ 1197 Hz
- KD2UGC F6GCP @ 472 Hz

## Next Steps

### Priority 1: Implement Signal Subtraction

**Tasks:**
1. Implement FT8 signal synthesis (message → tones → 12 kHz audio)
2. Add subtraction function (synthesized signal from original)
3. Modify decoder loop for multi-pass iterations
4. Add residual buffer management
5. Track subtracted signals to avoid re-processing

**Components needed:**
- `synthesize_ft8_signal(message, f0, dt, audio_buffer)`
- `subtract_signal(audio, synthesized, residual)`
- Multi-pass loop in `decoder.rs`

### Priority 2: Test on Other Failing Signals

Verify in-band interference is common failure mode across multiple frequency ranges.

### Priority 3: Re-enable nsym=2/3

After multi-pass works, fix phase tracking for multi-symbol combining to reach 22/22.

## Files Modified

### src/sync/downsample.rs
- Lines 73-147: Added comprehensive debug output
- Verified filter bin extraction
- Checked spectral power at key frequencies
- Analyzed filter sidelobe leakage
- **Status:** Debug output disabled after investigation complete

### Documentation Created
- [docs/inband_interference_root_cause.md](inband_interference_root_cause.md) - Full analysis
- [docs/session_20251125_part2.md](session_20251125_part2.md) - This document

### Documentation Updated
- [NEXT_STEPS.md](../NEXT_STEPS.md) - Updated root cause section, reprioritized tasks

## Technical Details

### Filter Math Verification

**For f0=2695 Hz:**
- NFFT_IN = 192000
- df = 12000 / 192000 = 0.0625 Hz/bin
- baud = 6.25 Hz
- fb = 2695 - 1.5×6.25 = 2685.625 Hz → bin 42970
- ft = 2695 + 8.5×6.25 = 2748.125 Hz → bin 43970
- i0 = 2695 Hz → bin 43120
- Extracted: 1001 bins, 62.6 Hz bandwidth ✓

**Interferer positions:**
- 2522 Hz → bin 40352 (2618 bins below ib=42970) - EXCLUDED ✓
- 2733 Hz → bin 43728 (within [42970, 43970]) - INCLUDED ⚠️

### Spectral Leakage Analysis

**32-point FFT properties:**
- Bin spacing: 200 Hz / 32 = 6.25 Hz (matches FT8 tone spacing)
- 8 bins for 8 tones (0-7)
- Rectangular window has poor sidelobe rejection (~-13 dB)

**W1DIG interference:**
- 38 Hz offset = 6.08 bins
- Creates sidelobes at target bins ±6
- Sidelobe power: -13 to -20 dB (still significant for weak signals)
- Weaker signal (SNR=-7) can overpower target (SNR=-3) during collisions

## Conclusion

This session definitively ruled out aliasing and filter bugs as the root cause. The problem is **legitimate in-band interference from overlapping FT8 signals** that requires **multi-pass decoding with signal subtraction** to resolve.

WSJT-X achieves 22/22 decodes specifically because it implements this multi-pass strategy. Without it, we are fundamentally limited to ~40% decode rate on realistic recordings with multiple simultaneous transmissions.

The path forward is clear: implement signal synthesis and multi-pass subtraction to match WSJT-X architecture.
