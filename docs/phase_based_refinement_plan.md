# Phase-Based Frequency Refinement Implementation Plan

## Goal

Reduce frequency error from 0.2 Hz to <0.05 Hz using phase progression of Costas arrays, enabling decode of F5RXL and similar signals.

**Expected impact**: +5-10 decodes (13-18/22 total, 59-82%)

---

## Background

**Current status**:
- F5RXL found at **1196.8 Hz** (actual: 1197 Hz, error: 0.2 Hz)
- Extraction: nsync=19/21 (90% Costas sync - excellent!)
- LLR: mean_abs_LLR=2.27 (good quality)
- LDPC: **FAILS** due to ~20% tone errors → 28% bit error rate

**Root cause**: 0.2 Hz = ~1 FFT bin at 0.195 Hz/bin resolution → wrong tones get peak power

---

## Algorithm

### Phase-Based Frequency Estimation

FT8 has 3 Costas arrays at known positions:
- Costas 1: symbols 0-6
- Costas 2: symbols 36-42
- Costas 3: symbols 72-78

Each Costas array is a fixed pattern: [3,1,4,0,6,5,2]

**Key insight**: If frequency is off, the phase will drift linearly over time.

**Formula**:
```
Δf = Δφ / (2π × Δt)
```

Where:
- `Δφ` = phase difference between Costas arrays
- `Δt` = time between Costas arrays

### Steps

1. **Initial extraction** at frequency `f0` (e.g., 1196.8 Hz)
   - Downsample signal at `f0`
   - Extract tones for all 79 symbols
   - Check Costas sync: `nsync >= 15/21` (good enough for phase measurement)

2. **Measure Costas phases**
   - For each of 3 Costas arrays:
     - Extract FFT of each symbol
     - Get complex value at expected tone bin
     - Calculate phase: `φ = atan2(imag, real)`
     - Average phase across 7 tones (weighted by power)

3. **Calculate phase drift**
   - Phase difference Costas1 → Costas2: `Δφ_1 = φ_2 - φ_1`
   - Phase difference Costas2 → Costas3: `Δφ_2 = φ_3 - φ_2`
   - Average: `Δφ = (Δφ_1 + Δφ_2) / 2`
   - Handle phase wrapping: if `|Δφ| > π`, adjust by ±2π

4. **Calculate frequency offset**
   - Time between Costas arrays: `Δt = 36 symbols × 0.16s/symbol = 5.76s`
   - Frequency offset: `Δf = Δφ / (2π × Δt)`
   - Refined frequency: `f_refined = f0 + Δf`

5. **Re-extract and decode**
   - Downsample at `f_refined`
   - Extract tones with improved accuracy
   - Compute LLRs
   - Attempt LDPC decode

---

## Implementation

### New Function in src/sync/extract.rs

```rust
/// Estimate frequency offset from Costas array phase progression
///
/// Measures the phase of Costas tones and calculates frequency offset
/// from phase drift over time. Returns refined frequency estimate.
///
/// # Arguments
/// * `signal` - Raw 12 kHz input signal
/// * `candidate` - Current candidate with initial frequency estimate
///
/// # Returns
/// * `Ok(refined_frequency)` if Costas sync is good enough (nsync >= 15/21)
/// * `Err(message)` if Costas sync is poor or phase measurement fails
pub fn estimate_frequency_from_phase(
    signal: &[f32],
    candidate: &Candidate,
) -> Result<f32, String> {
    // Downsample at current frequency estimate
    let mut cd = vec![(0.0f32, 0.0f32); 3200];
    downsample_200hz(signal, candidate.frequency, &mut cd)?;

    const NSPS: usize = 32; // 200 Hz × 0.16s = 32 samples per symbol
    const NFFT_SYM: usize = 32;
    const TONE_SPACING: f32 = 6.25; // Hz

    // Calculate start offset
    let dt = candidate.time_offset;
    let start_offset = ((dt + 0.5) * 200.0) as i32; // Convert to sample index

    // Measure phase for each Costas array
    let mut costas_phases = Vec::new();

    for costas_start in [0, 36, 72] {
        let mut phase_sum = 0.0;
        let mut weight_sum = 0.0;
        let mut valid_tones = 0;

        // Extract phase from each of 7 Costas tones
        for k in 0..7 {
            let symbol_idx = costas_start + k;
            let expected_tone = COSTAS_PATTERN[k];
            let i1 = start_offset + (symbol_idx as i32) * (NSPS as i32);

            // Check bounds
            if i1 < 0 || (i1 as usize + NSPS) > cd.len() {
                continue;
            }

            // Extract symbol
            let mut sym_real = [0.0f32; NFFT_SYM];
            let mut sym_imag = [0.0f32; NFFT_SYM];

            for j in 0..NSPS {
                let idx = (i1 as usize) + j;
                sym_real[j] = cd[idx].0;
                sym_imag[j] = cd[idx].1;
            }

            // Perform FFT
            if fft_real(&mut sym_real, &mut sym_imag, NFFT_SYM).is_err() {
                continue;
            }

            // Get phase at expected tone bin
            let tone_bin = expected_tone as usize;
            let re = sym_real[tone_bin];
            let im = sym_imag[tone_bin];
            let power = re * re + im * im;

            if power > 0.001 {
                let phase = im.atan2(re);
                phase_sum += phase * power; // Weighted by power
                weight_sum += power;
                valid_tones += 1;
            }
        }

        // Average phase for this Costas array
        if valid_tones >= 5 && weight_sum > 0.0 {
            costas_phases.push(phase_sum / weight_sum);
        } else {
            return Err(format!("Insufficient Costas tones for phase measurement"));
        }
    }

    // Need all 3 Costas arrays
    if costas_phases.len() < 3 {
        return Err(format!("Not enough Costas arrays detected"));
    }

    // Calculate phase differences (handle wrapping)
    let mut unwrap_phase = |p1: f32, p2: f32| -> f32 {
        let mut dp = p2 - p1;
        if dp > std::f32::consts::PI {
            dp -= 2.0 * std::f32::consts::PI;
        } else if dp < -std::f32::consts::PI {
            dp += 2.0 * std::f32::consts::PI;
        }
        dp
    };

    let dp1 = unwrap_phase(costas_phases[0], costas_phases[1]); // Costas1 → Costas2
    let dp2 = unwrap_phase(costas_phases[1], costas_phases[2]); // Costas2 → Costas3

    // Average phase drift
    let avg_phase_drift = (dp1 + dp2) / 2.0;

    // Calculate frequency offset
    // Time between Costas arrays: 36 symbols × 0.16s/symbol = 5.76s
    const DELTA_T: f32 = 36.0 * 0.16; // 5.76 seconds
    let freq_offset = avg_phase_drift / (2.0 * std::f32::consts::PI * DELTA_T);

    // Sanity check: offset should be < 1 Hz for 0.2 Hz error
    if freq_offset.abs() > 2.0 {
        return Err(format!("Unrealistic frequency offset: {:.3} Hz", freq_offset));
    }

    let refined_freq = candidate.frequency + freq_offset;

    Ok(refined_freq)
}
```

### Integration into Decoder

Modify `src/decoder.rs` to attempt phase-based refinement:

```rust
// After initial extraction shows good Costas sync
if nsync >= 15 {
    // Try phase-based frequency refinement
    if let Ok(refined_freq) = estimate_frequency_from_phase(signal, candidate) {
        let freq_correction = refined_freq - candidate.frequency;

        // Only apply if correction is reasonable (< 1 Hz)
        if freq_correction.abs() < 1.0 && freq_correction.abs() > 0.01 {
            // Create refined candidate
            let mut refined_candidate = candidate.clone();
            refined_candidate.frequency = refined_freq;

            // Re-extract at refined frequency
            let mut llr_diff_refined = [0.0f32; 174];
            let mut llr_ratio_refined = [0.0f32; 174];
            let mut s8_refined = [[0.0f32; 79]; 8];

            if extract_symbols_dual_llr(
                signal,
                &refined_candidate,
                nsym,
                &mut llr_diff_refined,
                &mut llr_ratio_refined,
                &mut s8_refined,
            ).is_ok() {
                // Try decoding with refined extraction
                // (existing LDPC loop with refined LLRs)
            }
        }
    }
}
```

---

## Expected Results

### Accuracy Improvement

**Before** (discrete 0.5 Hz search + interpolation):
- F5RXL: 1196.8 Hz (error: 0.2 Hz)
- K1BZM: 2695.3 Hz (error: 0.3 Hz)

**After** (phase-based refinement):
- F5RXL: ~1197.0 Hz (error: <0.05 Hz) ✓
- K1BZM: ~2695.0 Hz (error: <0.05 Hz) ✓

### Tone Error Reduction

**Before**: 0.2 Hz → ~20% tone errors → 28% bit error rate
**After**: <0.05 Hz → <10% tone errors → <15% bit error rate ✓ (within LDPC capability)

### Decode Rate

**Current**: 8/22 (36%)
**Expected**: 13-18/22 (59-82%)

**Newly decoded signals**:
- CQ F5RXL IN94 @ 1197 Hz (SNR=-2 dB)
- K1BZM EA3GP @ 2695 Hz (SNR=-3 dB)
- N1PJT HB9CQK @ 466 Hz (SNR=-2 dB)
- ...and 5-10 more

---

## Testing Strategy

### Unit Test

```rust
#[test]
fn test_phase_based_frequency_refinement() {
    // Synthesize FT8 signal at 1197 Hz
    // Estimate frequency starting from 1196.8 Hz
    // Assert refined frequency within 0.05 Hz of 1197 Hz
}
```

### Integration Test

Modify `test_real_ft8_recording_210703_133430`:
- Enable phase-based refinement
- Check that F5RXL decodes
- Verify decode count increases to 13-18/22

### Debug Output

Add logging to show:
```
PHASE_REFINE: freq_initial=1196.8 Hz
  Costas phases: φ1=1.234, φ2=2.456, φ3=3.678
  Phase drift: Δφ=0.723 rad, Δt=5.76s
  Freq offset: Δf=+0.020 Hz
  Refined: freq_final=1196.8 Hz
```

---

## Alternative: Simpler Approach

If phase measurement proves difficult, consider:

### Option A: Wider FFT Bins

Change from 32-point to 64-point FFT:
- Resolution: 0.098 Hz/bin (vs 0.195 Hz/bin)
- 0.2 Hz error = 2 bins (vs 1 bin)
- More robust to frequency errors

**Pros**: Simpler, no iteration
**Cons**: 2x compute, slight loss in time resolution

### Option B: Finer Search Grid

Fine sync with 0.25 Hz steps (vs 0.5 Hz):
- 21 test frequencies (vs 11)
- Could find 1197.0 Hz directly

**Pros**: No algorithm changes
**Cons**: 2x compute, still discrete

---

## References

- [f5rxl_final_bottleneck_analysis.md](f5rxl_final_bottleneck_analysis.md) - Root cause analysis
- [tone_extraction_root_cause.md](tone_extraction_root_cause.md) - Why 0.2 Hz causes 20% errors
- [sync2d_fix_breakthrough_20251125.md](sync2d_fix_breakthrough_20251125.md) - Sync2d fix details
- WSJT-X ft8b.f90 - Reference implementation (no phase refinement found)

---

## Implementation Checklist

- [ ] Add `estimate_frequency_from_phase()` to src/sync/extract.rs
- [ ] Export function in src/sync/mod.rs
- [ ] Integrate into decoder.rs after initial extraction
- [ ] Add unit test for phase measurement
- [ ] Test on F5RXL signal (expect 1197.0 Hz ± 0.05 Hz)
- [ ] Run full test suite (expect 13-18/22 decodes)
- [ ] Document results
- [ ] Benchmark performance impact (<10% overhead expected)

---

## Notes

**Phase measurement reliability**:
- Requires nsync >= 15/21 (at least 5 tones per Costas array)
- Power-weighted average reduces noise impact
- Sanity checks prevent wild corrections

**Computational cost**:
- Phase measurement: 3 × 7 × FFT(32) = 21 FFTs
- Re-extraction if refined: 1 full extraction
- Total overhead: ~5-10% (only for candidates with good Costas sync)

**When to apply**:
- Only if initial nsync >= 15/21
- Only if |Δf| < 1 Hz (sanity check)
- Only if |Δf| > 0.01 Hz (avoid unnecessary re-extraction)

---

## Success Criteria

1. ✅ Phase measurement succeeds for F5RXL (nsync=19/21)
2. ✅ Refined frequency within 0.05 Hz of WSJT-X's 1197 Hz
3. ✅ F5RXL decodes after refinement
4. ✅ No regressions (still 8/22 minimum, expect 13-18/22)
5. ✅ <10% performance overhead
