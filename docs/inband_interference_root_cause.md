# In-Band Interference Root Cause - 2025-11-25

## Critical Discovery

The tone extraction errors for K1BZM EA3GP @ 2695 Hz are caused by **in-band interference from another FT8 signal**, NOT by aliasing or filter bugs.

## The Problem

### Interfering Signal

**W1DIG SV9CVY @ 2733 Hz** (SNR=-7 dB per WSJT-X)
- Only **38 Hz away** from target (6.08 FT8 tones)
- Power=29.96 (comparable to target power=34.67)
- **Both signals are legitimately in the same 62.6 Hz FT8 passband**

### Filter Behavior

Our frequency-domain filter extracts [f0-9.375, f0+53.125] Hz = 62.5 Hz bandwidth:
- For target @ 2695 Hz: extracts [2685.9, 2748.4] Hz
- Target EA3GP @ 2695 Hz: **IN RANGE** ✓
- Interferer W1DIG @ 2733 Hz: **IN RANGE** ✓
- This is **CORRECT** per FT8 protocol specification

### Why Tone Extraction Fails

After downsampling to 200 Hz baseband:
- EA3GP is centered at ~0 Hz
- W1DIG is offset by ~38 Hz (6.08 tones)

Symbol extraction uses 32-point FFT with 8 bins (one per tone):
- Bin spacing: 200 Hz / 32 = 6.25 Hz per bin
- W1DIG offset: 38 Hz / 6.25 Hz = 6.08 bins

**The two signals are only 6 tones apart**, causing spectral leakage:
- When W1DIG has a tone at position N, it creates sidelobes at N±6
- These sidelobes pollute EA3GP's tone extraction
- Wrong FFT bins get 5x-17x higher power during collision symbols

## Verification

### Debug Output Confirms

```
=== DOWNSAMPLE DEBUG for f0=2695.3 Hz ===
  Filter range: fb=2685.9 Hz → ib=42975, ft=2748.4 Hz → it=43975
  Spectral power check (INPUT FFT):
    2695.0 Hz (Target EA3GP): bin=43120, power=3.467e1, extracted=true
    2733.0 Hz (Interferer W1DIG (INSIDE passband!)): bin=43728, power=2.996e1, extracted=true
```

Both signals have comparable power and are both extracted - this is correct behavior.

### WSJT-X Decodes Both

```
133430  -3 -0.1 2695 ~  K1BZM EA3GP -09
133430  -7  0.4 2733 ~  W1DIG SV9CVY -14
```

WSJT-X successfully decodes both signals at the same time using multi-pass decoding with signal subtraction.

## Why Our Previous Hypothesis Was Wrong

### Initial Aliasing Theory

We thought the interferer at 2522 Hz (173 Hz away, outside Nyquist) was aliasing into the passband. **This was incorrect**:
- 2522 Hz signal is **excluded** from [2685.9, 2748.4] Hz range
- 2618 bins (163.9 Hz) away from filter edge
- `extracted=false` confirms it's filtered out

### Actual Root Cause

The interferer is **inside the passband** at 2733 Hz, only 38 Hz away. This is a **legitimate multi-signal scenario** that requires multi-pass decoding, not a filter bug.

## Why Errors Cluster at Symbols 30-35

### Tone Collision Analysis

Errors occur when both signals have tones that create maximum interference:
- EA3GP symbol 30: tone 3
- W1DIG symbol 30: tone ~2 or ~4 (6 tones offset creates ambiguity)

When both signals transmit simultaneously and their tones are close (within ~2-3 bins), the 32-point FFT cannot resolve them cleanly.

### Why Costas Arrays Are Perfect

Costas arrays use fixed patterns (3,1,4,0,6,5,2):
- Known tone sequence reduces ambiguity
- Stronger sync power (by design)
- Less susceptible to spectral leakage
- Fine sync optimizes for Costas correlation specifically

Data symbols have arbitrary tones and weaker power, making them more vulnerable to interference.

## The Solution: Multi-Pass Subtraction

### WSJT-X Strategy

From `ft8b.f90`:
```fortran
if(lsubtract) then
   call timer('sub_ft8a',0)
   call subtractft8(dd0,itone,f1,xdt,.false.)
   call timer('sub_ft8a',1)
endif
```

**Multi-pass decoding:**
1. **Pass 1**: Decode all candidates with current audio
2. For each successful decode:
   - Synthesize the FT8 signal from decoded message
   - Subtract it from the original 12 kHz audio
3. **Pass 2**: Decode again on residual audio
4. Repeat until no new decodes found

### Why This Works

**Strong signals decoded first:**
- EA3GP @ 2695 Hz, SNR=-3 dB (stronger)
- Decode and subtract from audio

**Residual reveals weak signal:**
- W1DIG @ 2733 Hz, SNR=-7 dB (weaker)
- Now visible without EA3GP interference
- Decode from clean residual

## Implementation Plan

### Priority 1: Implement Signal Subtraction

We already have `src/message/subtract.rs` with the framework. Need to:
1. Generate synthesized FT8 signal from decoded message
2. Subtract from original 12 kHz audio buffer
3. Re-run candidate detection and decoding on residual
4. Iterate until no new decodes

### Priority 2: Multi-Pass Decode Loop

Modify decoder.rs to:
1. Store original audio buffer
2. After each successful decode, subtract signal
3. Re-detect candidates on residual
4. Decode new candidates
5. Repeat 2-4 until convergence

### Expected Improvement

- **Current**: 9/22 decodes (41%)
- **After subtraction**: 18-20/22 decodes (82-91%)
- **Target**: 22/22 decodes (100%) matching WSJT-X

### Other Signals Likely Affected

Check these failing decodes for in-band interference:
- N1PJT HB9CQK @ 466 Hz (check 400-530 Hz range)
- CQ F5RXL @ 1197 Hz (check 1140-1260 Hz range)
- KD2UGC F6GCP @ 472 Hz (check 400-540 Hz range)

## Conclusion

The problem is **NOT**:
- ❌ Aliasing during downsampling
- ❌ Filter bug or bin extraction error
- ❌ LLR computation issues
- ❌ LDPC decoder problems

The problem **IS**:
- ✅ Legitimate in-band interference from nearby FT8 signals
- ✅ Missing multi-pass subtraction that WSJT-X uses
- ✅ Need to handle overlapping signals in same passband

This is a **decoder architecture issue**, not a signal processing bug. WSJT-X achieves 22/22 decodes specifically because it uses iterative subtraction to separate overlapping signals.

## References

- `wsjtx/wsjtx-2.7.0/src/wsjtx/lib/ft8/ft8b.f90` - Multi-pass decoding with subtraction
- `docs/wsjtx_multipass_strategy.md` - Previous analysis of WSJT-X approach
- `src/message/subtract.rs` - Our existing (unused) subtraction module
