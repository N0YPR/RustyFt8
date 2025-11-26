# How WSJT-X Handles Nearby Signal Interference

## Date: 2025-11-25

## WSJT-X's Approach

### Signal Subtraction

From `ft8b.f90`:

```fortran
if(lsubtract) then
   call timer('sub_ft8a',0)
   call subtractft8(dd0,itone,f1,xdt,.false.)
   call timer('sub_ft8a',1)
endif
```

WSJT-X **does** use signal subtraction:
1. After each successful decode, the signal is subtracted from the original audio
2. Subsequent decode attempts work on the residual
3. This prevents strong signals from interfering with weaker ones

### Decode Order

WSJT-X likely processes candidates in order of sync power:
- Strongest signals decoded first
- Weaker signals decoded from residual after subtraction

For our case:
- K1BZM EA3GP @ 2695 Hz: SNR=-3 dB (stronger)
- K1BZM EA3CJ @ 2522 Hz: SNR=-7 dB (weaker)

EA3GP would be attempted first, but it **fails** in our decoder due to interference from EA3CJ. In WSJT-X, the narrowband filtering might be better, OR they rely on a different mechanism.

## Frequency-Domain Filtering

Both WSJT-X and RustyFt8 use the same approach:
1. FFT the full 15s recording at 12 kHz
2. Extract bins [f0-9.375, f0+53.125] Hz (62.5 Hz bandwidth)
3. Inverse FFT to get 200 Hz sample rate

For f0=2695 Hz:
- Extract bins: 2685.6 to 2748.1 Hz
- EA3CJ at 2522 Hz is at bin 40352
- Target range: bins 42970-43970
- **Distance: 2618 bins (163 Hz away)**

The interferer **should be filtered out** by the frequency-domain extraction.

## The Puzzle

If WSJT-X uses the same filter, why do they succeed while we fail?

### Hypothesis 1: Our Filter Has a Bug

Possible issues in our `downsample_200hz`:
1. **Bin indexing off-by-one**: We might extract wrong bins
2. **Negative frequency handling**: Real FFT has conjugate pairs at negative frequencies
3. **Circular shift bug**: Not properly centering DC
4. **Filter sidelobes**: Significant energy leaking from 163 Hz away

### Hypothesis 2: It's Not Actually Aliasing

Alternative explanations for the tone errors:
1. **Timing drift**: Accumulated phase error over symbols 30-35
2. **Frequency offset**: Small offset causing FFT bin spreading
3. **Actual nearby signal**: Different interferer we haven't identified
4. **Decoder artifact**: Something in our symbol extraction path

### Hypothesis 3: WSJT-X Uses Subtraction

Even with good filtering, there might be filter sidelobe leakage. WSJT-X might:
1. Decode both signals
2. Use subtraction to clean up residual interference
3. Re-decode with better SNR

## Key Difference: WSJT-X Has Sharp Filters

Looking at their downsampling:
- Uses Fortran FFT libraries (FFTW or similar)
- May have different filter characteristics
- Taper function might provide sharper rolloff

## Next Steps to Verify

1. **Add debug output to downsample**:
   - Show exactly which bins [ib, it] are extracted
   - Check if EA3CJ at bin 40352 is being copied
   - Verify shift and normalization

2. **Compare filter shapes**:
   - Plot our extracted spectrum
   - Compare with WSJT-X's expected filter shape
   - Check for sidelobe energy

3. **Test signal subtraction**:
   - We already have `subtract.rs` module
   - Enable multi-pass decoding
   - See if EA3GP decodes after EA3CJ is subtracted

4. **Verify aliasing hypothesis**:
   - Create synthetic test: 2695 Hz + 2522 Hz interferer
   - Check if we see the same error pattern
   - This would confirm or refute aliasing

## Conclusion

WSJT-X likely uses **both** approaches:
1. **Good frequency-domain filtering** (should exclude signals >30 Hz away)
2. **Signal subtraction** (handles any residual interference)

Our issue is likely:
- **Filter implementation bug** (extracting wrong bins or poor sidelobe rejection)
- OR **Need multi-pass with subtraction** to match WSJT-X

The path forward:
1. Debug our filter first (verify bin extraction)
2. If filter is correct, enable multi-pass subtraction
3. Compare results with WSJT-X
