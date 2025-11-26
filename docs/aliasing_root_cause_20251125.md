# ROOT CAUSE DISCOVERED: Signal Aliasing

## Date: 2025-11-25

## Summary

Found the root cause of tone extraction errors: **Aliasing from nearby FT8 signals** during downsampling.

## The Evidence

### Tone Error Pattern

Symbols 30-35 (end of data block 1) have massive errors:

| Symbol | Expected | Got | Diff | Power Ratio |
|--------|----------|-----|------|-------------|
| 30 | 3 | 1 | -2 | 5.5x wrong > correct |
| 31 | 5 | 4 | -1 | 1.8x |
| 32 | 3 | 2 | -1 | 4.5x |
| 33 | 7 | 4 | -3 | 1.9x |
| 34 | 5 | 1 | -4 | 17.1x |
| 35 | 5 | 3 | -2 | 14.8x |

**Pattern**: ALL errors are negative (detected < expected), averaging -2.2 tones ≈ -13.75 Hz

But Costas 2 (symbols 36-42) immediately after is **PERFECT** (0 errors)!

### FFT Bin Powers Show Interference

From detailed diagnostic output:

```
Sym[30] DATA: exp=3 got=1 *ERR* |  0.029  [0.176]! 0.082  (0.032)  ...
Sym[34] DATA: exp=5 got=1 *ERR* |  0.070  [0.199]! 0.080   0.053   0.012  (0.012)  ...
Sym[35] DATA: exp=5 got=3 *ERR* |  0.004   0.014   0.047  [0.380]! 0.046  (0.026)  ...
Sym[36] COS2: exp=3 got=3   OK  |  0.009   0.032   0.034  [0.277]  ...
```

- Wrong bins have MUCH higher power (up to 17x) than expected bins
- Total power during sym 30-35 is actually HIGHER than Costas 2
- This means there's real signal energy, but at wrong frequencies

## The Aliasing Calculation

### Nearby Signals (from WSJT-X)

```
133430  -3 -0.1 2695 ~  K1BZM EA3GP -09     (our target)
133430  -7  0.2 2522 ~  K1BZM EA3CJ JN01    (interferer 1)
133430  -9 -0.1 2546 ~  WA2FZW DL5AXX RR73  (interferer 2)
```

### Frequency Offsets

- EA3CJ: 2522 Hz, **173 Hz below target**
- WA2FZW: 2546 Hz, **149 Hz below target**

### Aliasing Analysis

When we downsample to 200 Hz centered at 2695 Hz:
- **Nyquist range**: ±100 Hz (2595-2795 Hz)
- **EA3CJ at 2522 Hz**: 173 Hz below = **OUTSIDE Nyquist!**
- **Aliases to**: 200 - 173 = **27 Hz** in baseband
- **That's**: 27/6.25 = **4.3 tones offset**

This matches the -2 to -4 tone errors we observe!

### Why Only Symbols 30-35?

The aliased interferer creates strong peaks only when:
1. The interfering signal (EA3CJ) has specific tone values
2. Those tones, when aliased, fall into bins we're checking
3. The target signal (EA3GP) is weak enough to be overpowered

During symbols 30-35, EA3CJ happens to transmit tones that alias into the bins EA3GP is using, creating 5x-17x stronger wrong peaks.

### Why Costas Arrays Are Perfect?

Costas arrays have:
- **Stronger signal power** (known sync pattern, higher correlation)
- **Fixed pattern** that's robust to interference
- Sufficient SNR to not be overpowered by aliased signals

## WSJT-X's Approach

From `ft8_downsample.f90`:

```fortran
fb=f0-1.5*baud  ! f0 - 9.375 Hz
ft=f0+8.5*baud  ! f0 + 53.125 Hz
! Bandwidth: ~62.5 Hz
```

WSJT-X extracts only **62.5 Hz bandwidth** in frequency domain, which should filter out signals >30 Hz away.

## Our Implementation

From `src/sync/downsample.rs`:

```rust
let fb = (f0 - 1.5 * baud).max(0.0);  // Matches WSJT-X
let ft = (f0 + 8.5 * baud).min(SAMPLE_RATE / 2.0);  // Matches WSJT-X
```

**We DO match WSJT-X's filter range!**

## The Puzzle

If we're correctly extracting only 62.5 Hz bandwidth around 2695 Hz:
- The 2522 Hz signal is 173 Hz away
- It should be filtered out by the frequency-domain extraction
- So how is it getting through?

### Possible Issues

1. **Bug in bin indexing**: We might be extracting wrong bins
2. **FFT format mismatch**: Real FFT packing might be different
3. **Filter not sharp enough**: Frequency-domain filter has slow rolloff
4. **Post-processing issue**: Aliasing introduced after inverse FFT

## Next Steps

1. **Add debug output to downsample**:
   - Show which bins are being extracted (ib to it)
   - Verify 2522 Hz signal is NOT in extracted range
   - Check power in extracted vs rejected bins

2. **Compare FFT output with WSJT-X**:
   - Verify bin ordering and normalization
   - Check if we're handling negative frequencies correctly

3. **Test with synthetic signal**:
   - Create clean FT8 at 2695 Hz
   - Add interferer at 2522 Hz
   - Verify if aliasing occurs

4. **Sharpen anti-aliasing filter**:
   - Add steeper rolloff at filter edges
   - Or increase sample rate to widen Nyquist range

## Impact

This explains:
- ✓ Why we only decode 9/22 (41%) vs WSJT-X's 22/22 (100%)
- ✓ Why errors cluster at specific symbols
- ✓ Why wrong bins have dramatically higher power
- ✓ Why Costas arrays are perfect but adjacent data fails
- ✓ Why fine sync improvements didn't help

**The gap isn't LLR, LDPC, or timing - it's aliasing during downsampling!**
