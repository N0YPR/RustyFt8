# Signal Subtraction Implementation Plan

## WSJT-X Algorithm (from subtractft8.f90 and gen_ft8wave.f90)

### Step 1: Generate Reference Signal
```fortran
call gen_ft8wave(itone, 79, 1920, 2.0, 12000.0, f0, cref, ...)
```

**Inputs:**
- `itone(79)`: Decoded tone sequence (values 0-7)
- `nsps=1920`: Samples per symbol at 12 kHz
- `bt=2.0`: Gaussian filter BT product
- `fsample=12000.0`: Sample rate
- `f0`: Carrier frequency (Hz)

**Process:**
1. Generate Gaussian pulse for frequency smoothing
2. Compute instantaneous frequency for each sample:
   - `dphi(t) = 2π * hmod * pulse(t) * itone[symbol]`
   - Add carrier: `dphi(t) += 2π * f0 * dt`
3. Integrate to get phase: `phi(t) = ∫ dphi(t) dt`
4. Generate complex exponential: `cref(t) = exp(j*phi(t))`
5. Apply envelope shaping to first/last symbols

### Step 2: Estimate Complex Amplitude
```fortran
camp(t) = dd(t) * conjg(cref(t))  ! Mix down to baseband
cfilt(t) = LPF[camp(t)]            ! Low-pass filter
```

**Filter specs:**
- Window: `cos²(π*t/NFILT)` where NFILT=4000 samples
- FFT-based convolution for efficiency
- End correction for window edge effects

### Step 3: Reconstruct and Subtract
```fortran
reconstructed(t) = 2 * Real{cref(t) * cfilt(t)}
dd(t) = dd(t) - reconstructed(t)
```

**Optional refinement:**
- Search ±90 samples for better time alignment
- Use spectral residual metric to find optimal offset

## Rust Implementation Architecture

### File Structure
```
src/
├── symbol/
│   ├── mod.rs
│   ├── generate.rs      # NEW: gen_ft8_waveform()
│   └── subtract.rs      # NEW: subtract_ft8_signal()
├── pulse.rs             # Already exists: GFSK pulse
└── decoder.rs           # MODIFY: Add multi-pass with subtraction
```

### Phase 1: Tone-to-Waveform Generation

**Function signature:**
```rust
/// Generate FT8 reference waveform from tone sequence
///
/// # Arguments
/// * `tones` - 79-element tone sequence (0-7)
/// * `frequency` - Carrier frequency in Hz
/// * `sample_rate` - Sample rate (typically 12000 Hz)
/// * `output` - Output buffer for complex waveform
pub fn generate_ft8_waveform(
    tones: &[u8; 79],
    frequency: f32,
    sample_rate: f32,
    output: &mut [(f32, f32)],  // Complex samples (re, im)
) -> Result<(), String>
```

**Implementation steps:**
1. ✅ Use existing `gfsk_pulse()` from pulse.rs
2. Compute instantaneous frequency per sample
3. Integrate to phase
4. Generate complex exponential (lookup table for efficiency)
5. Apply envelope shaping

**Complexity:** O(N) where N = 79 * 1920 = 151,680 samples

### Phase 2: Signal Subtraction

**Function signature:**
```rust
/// Subtract a decoded FT8 signal from audio
///
/// # Arguments
/// * `audio` - Input/output audio buffer (modified in-place)
/// * `tones` - Decoded tone sequence
/// * `frequency` - Signal frequency in Hz
/// * `time_offset` - Signal time offset in seconds
/// * `sample_rate` - Sample rate (12000 Hz)
pub fn subtract_ft8_signal(
    audio: &mut [f32],
    tones: &[u8; 79],
    frequency: f32,
    time_offset: f32,
    sample_rate: f32,
) -> Result<(), String>
```

**Implementation steps:**
1. Generate reference signal `cref(t)` via generate_ft8_waveform()
2. Extract signal window starting at `time_offset`
3. Mix down: `camp(t) = audio(t) * conj(cref(t))`
4. Low-pass filter `camp(t)` → `cfilt(t)`
5. Reconstruct: `reconstructed(t) = 2 * Re{cref(t) * cfilt(t)}`
6. Subtract from audio in-place

**Filter implementation:**
- Use FFT-based convolution
- Cosine-squared window (NFILT = 4000 samples)
- Apply end corrections

**Complexity:** O(N log N) for FFT convolution

### Phase 3: Multi-Pass Decoding

**Modify decoder.rs:**
```rust
pub fn decode_ft8_multipass<F>(
    signal: &[f32],
    config: &DecoderConfig,
    callback: F,
) -> Result<usize, &'static str>
where
    F: FnMut(DecodedMessage) -> bool,
{
    let mut working_signal = signal.to_vec();
    let mut total_decodes = 0;

    // Pass 1: Initial decode
    let pass1_decodes = decode_pass(&working_signal, config, &mut callback)?;
    total_decodes += pass1_decodes;

    // Subtract decoded signals
    for decoded in &pass1_messages {
        subtract_ft8_signal(
            &mut working_signal,
            &decoded.tones,
            decoded.frequency,
            decoded.time_offset,
            12000.0,
        )?;
    }

    // Pass 2: Decode revealed signals
    let pass2_decodes = decode_pass(&working_signal, config, &mut callback)?;
    total_decodes += pass2_decodes;

    // Optional Pass 3 (high depth only)
    if config.depth >= 3 && pass2_decodes > 0 {
        // Subtract pass 2 signals...
        let pass3_decodes = decode_pass(&working_signal, config, &mut callback)?;
        total_decodes += pass3_decodes;
    }

    Ok(total_decodes)
}
```

## Implementation Order

### Sprint 1: Waveform Generation (4-6 hours)
1. Implement `generate_ft8_waveform()`
2. Add phase accumulator with lookup table
3. Apply envelope shaping
4. Unit tests comparing to WSJT-X output

### Sprint 2: Signal Subtraction (4-6 hours)
1. Implement FFT-based low-pass filter
2. Implement `subtract_ft8_signal()`
3. Add time offset handling
4. Unit tests with synthetic signals

### Sprint 3: Multi-Pass Integration (2-4 hours)
1. Modify decoder.rs for multi-pass
2. Add decoded message tracking
3. Integrate subtraction between passes
4. Test on real recording

### Sprint 4: Optimization (2-3 hours)
1. Optimize FFT operations
2. Consider parallelization
3. Profile and tune performance

## Expected Results

### After Sprint 1-2
- Signal subtraction functional
- Able to remove decoded signals from audio

### After Sprint 3
- **Expected: 14-18 decodes** (current 6 + 8-12 from subtraction)
- Validates MULTIPASS_ANALYSIS.md predictions

### After Sprint 4
- Performance comparable to WSJT-X
- ~2-3 seconds total decode time

## Testing Strategy

### Unit Tests
1. **Waveform generation**: Compare to WSJT-X ft8code output
2. **Subtraction**: Synthetic signal, verify residual < -30 dB
3. **Multi-pass**: Known recording, track decode progression

### Integration Tests
1. **Real recording**: 210703_133430.wav
2. **Target**: 14-18 decodes (vs WSJT-X's 22)
3. **Verify**: All decoded messages match WSJT-X output

## References

- WSJT-X: `lib/ft8/subtractft8.f90`
- WSJT-X: `lib/ft8/gen_ft8wave.f90`
- FT8 Protocol: https://wsjt.sourceforge.io/FT4_FT8_QEX.pdf
