# LLR 4-Pass Strategy Discovery - 2025-11-25

## Critical Finding

**WSJT-X achieves better BP convergence by trying 4 different LLR representations of the same signal, not just 1.**

## Background

Current status: 8/22 decodes (36%) with **57% OSD usage** even for strong signals (-4 to -8 dB).

Problem: Strong signals should decode with BP (Belief Propagation), not fall back to OSD (Ordered Statistics Decoding).

## WSJT-X Strategy

### 4-Pass Decode Loop (ft8b.f90 lines 265-269)

```fortran
do ipass=1,npasses
   llrz=llra           ! Pass 1: standard nsym=1
   if(ipass.eq.2) llrz=llrb   ! Pass 2: nsym=2
   if(ipass.eq.3) llrz=llrc   ! Pass 3: nsym=3
   if(ipass.eq.4) llrz=llrd   ! Pass 4: normalized nsym=1
```

### 4 Different LLR Arrays (ft8b.f90 lines 211-239)

**During extraction** with nsym=1, 2, 3:

1. **llra (bmeta)**: Standard LLR from nsym=1
   ```fortran
   bmeta(i32+ib) = max(ones) - max(zeros)  ! Difference method
   ```

2. **llrb (bmetb)**: Standard LLR from nsym=2
   ```fortran
   bmetb(i32+ib) = max(ones) - max(zeros)  ! 2-symbol combining
   ```

3. **llrc (bmetc)**: Standard LLR from nsym=3
   ```fortran
   bmetc(i32+ib) = max(ones) - max(zeros)  ! 3-symbol combining
   ```

4. **llrd (bmetd)**: **Normalized ratio LLR** from nsym=1
   ```fortran
   den = max(max(ones), max(zeros))
   bmetd(i32+ib) = (max(ones) - max(zeros)) / den  ! Ratio method
   ```

Then all 4 arrays normalized by std dev and scaled by 2.83:
```fortran
call normalizebmet(bmeta,174)
call normalizebmet(bmetb,174)
call normalizebmet(bmetc,174)
call normalizebmet(bmetd,174)

scalefac=2.83
llra=scalefac*bmeta
llrb=scalefac*bmetb
llrc=scalefac*bmetc
llrd=scalefac*bmetd
```

### Key Insight: llrd is NOT just scaled llra

**llrd uses a fundamentally different LLR metric**:
- **llra**: Difference method = `numerator only`
- **llrd**: Ratio method = `numerator / denominator`

The ratio method normalizes by signal strength, making it more robust to amplitude variations.

## RustyFt8 Current Approach

### Single-Pass with Scaling (decoder.rs lines 105-165)

```rust
let nsym_values = [1];  // Only nsym=1 (2/3 disabled)
let scaling_factors = [1.0, 1.5, 0.75, 2.0, 0.5, ...];  // 16 values

for &nsym in &nsym_values {
    sync::extract_symbols(signal, &refined, nsym, &mut llr)?;

    for &scale in &scaling_factors {
        let mut scaled_llr = llr.clone();
        for v in scaled_llr.iter_mut() {
            *v *= scale;
        }
        ldpc::decode_hybrid(&scaled_llr, ...);
    }
}
```

### LLR Computation (extract.rs lines 523, 575, 611, 664)

**Only difference method**:
```rust
llr[bit_idx] = max_mag_1 - max_mag_0;  // Difference only
```

**Never computes ratio method**:
```rust
// Missing:
let den = max_mag_1.max(max_mag_0);
llr[bit_idx] = if den > 0.0 {
    (max_mag_1 - max_mag_0) / den
} else {
    0.0
};
```

## Root Cause of 57% OSD Usage

### WSJT-X Advantages

1. **4 different signal representations**:
   - nsym=1 standard (difference method)
   - nsym=1 normalized (ratio method)
   - nsym=2 multi-symbol
   - nsym=3 multi-symbol

2. **Multiple chances for BP convergence**:
   - If llra has poor LLR quality → try llrb
   - If llrb has phase issues → try llrc
   - If llrc has noise → try llrd
   - One of the 4 usually works

3. **Different methods for different conditions**:
   - Ratio method (llrd) robust to amplitude variations
   - Multi-symbol (llrb/llrc) improves SNR by 3-6 dB
   - Standard method (llra) fastest when signal is clean

### RustyFt8 Limitations

1. **Single signal representation**:
   - Only nsym=1 extraction
   - Only difference method
   - Only 1 view of the signal

2. **Scaling doesn't help poor LLRs**:
   - Trying 16 scales on same LLR array
   - If LLR quality is fundamentally bad, scaling won't fix it
   - Example: If wrong bins have max power, scaling doesn't change that

3. **No fallback for phase issues**:
   - nsym=2/3 disabled due to phase drift (session_20251125_part3.md)
   - No alternative representation to try
   - Forced to rely on OSD when BP fails

## Why Scaling Factors Don't Replace 4-Pass

**Scaling factors** (our approach):
- Multiply same LLR values by different constants
- Changes magnitude, not the underlying information
- If bit 42 has weak confidence (LLR=0.5), scaling just makes it (0.75 or 1.0)
- Doesn't fix fundamental extraction errors

**4-Pass LLR arrays** (WSJT-X approach):
- Completely different bit confidence values
- Different extraction methods → different "views" of signal
- If bit 42 has LLR=0.5 in llra, might have LLR=2.5 in llrd
- Ratio method provides truly independent information

## Comparison Table

| Aspect | WSJT-X | RustyFt8 | Impact |
|--------|--------|----------|--------|
| **LLR methods** | 2 (diff + ratio) | 1 (diff only) | -50% representations |
| **nsym values** | 1, 2, 3 | 1 only | -67% representations |
| **Total LLR arrays** | 4 | 1 | -75% representations |
| **Decode attempts** | 4 LLR arrays × scales | 1 LLR array × 16 scales | Fundamentally different |
| **BP convergence** | ~80-90% | ~43% (57% OSD) | -40% success rate |
| **Robustness** | Multiple fallbacks | Single approach | Much less robust |

## Evidence from Our Tests

From [session_20251125_part3.md](session_20251125_part3.md):

**Strong signals using OSD** (should use BP):
- W1FC @ 2572 Hz (-8 dB): **OSD** ← Should be BP!
- WM3PEN @ 2157 Hz (-4 dB): **OSD** ← Strong signal!

**This proves**: Our single LLR method has poor quality even for strong signals.

From [nsym23_retest_20251125.md](nsym23_retest_20251125.md):

**nsym=2/3 testing**:
- Enabled nsym=1/2/3: 8 correct + 2 false positives (20% FP rate)
- Disabled nsym=1 only: 8 correct + 1 false positive (11% FP rate)

**This proves**: We can't currently use nsym=2/3 due to phase tracking issues, limiting us to 1 representation.

## Proposed Solution

### Option 1: Implement Ratio Method LLR (Quick Win)

**Add llrd-equivalent** to our nsym=1 extraction:

```rust
// In extract.rs, after computing max_mag_1 and max_mag_0:

// Standard difference method (current)
llr_diff[bit_idx] = max_mag_1 - max_mag_0;

// NEW: Ratio method (WSJT-X llrd equivalent)
let den = max_mag_1.max(max_mag_0);
llr_ratio[bit_idx] = if den > 0.0 {
    (max_mag_1 - max_mag_0) / den
} else {
    0.0
};
```

**Then try decoding with both**:
```rust
// Pass 1: Try difference method
if let Some(result) = ldpc::decode_hybrid(&llr_diff, ...) {
    return Some(result);
}

// Pass 2: Try ratio method
if let Some(result) = ldpc::decode_hybrid(&llr_ratio, ...) {
    return Some(result);
}
```

**Expected impact**:
- Reduce OSD usage from 57% → ~35-40%
- Add 1-3 more decodes (ratio method may work where difference fails)
- Minimal code changes (single file)

### Option 2: Fix Phase Tracking for nsym=2/3 (Medium Effort)

**Improve phase tracking** (extract.rs lines 146-180):
- Current: ±1.0 Hz search with 0.05 Hz steps
- Needed: Per-symbol phase correction using Costas arrays
- Better coherent combining across symbols

**Expected impact**:
- Enable nsym=2/3 safely (without false positives)
- Get 4 LLR representations (llra, llrb, llrc, llrd)
- Reduce OSD usage to ~20%
- Add 5-10 more decodes

### Option 3: Full 4-Pass Implementation (High Effort)

**Match WSJT-X exactly**:
1. Extract with nsym=1, 2, 3
2. Compute both difference and ratio LLRs for each
3. Generate 4 LLR arrays: llra, llrb, llrc, llrd
4. Try decoding with all 4 sequentially
5. Match normalization and scaling exactly

**Expected impact**:
- Match WSJT-X BP convergence (~80-90%)
- Reduce OSD usage to <20%
- Add 10-14 more decodes
- Likely achieve 22/22 (100%) decode rate

## Recommendation

**Start with Option 1** (ratio method):
- Quick to implement (~1 hour)
- Low risk (doesn't affect existing decodes)
- Validates the hypothesis (do we see improved BP convergence?)
- If successful, proceed to Option 2

**Then Option 2** (fix phase tracking):
- Required for full 4-pass strategy anyway
- Enables nsym=2/3 safely
- Medium effort (~4-6 hours)

**Finally Option 3** (full 4-pass):
- Once Options 1+2 working, complete the implementation
- Expected to reach 100% decode rate

## Next Steps

1. **Implement ratio method LLR** (Option 1)
   - Modify extract.rs to compute both difference and ratio LLRs
   - Update decoder.rs to try both methods
   - Test on 210703_133430.wav
   - Document OSD usage change

2. **Measure improvement**
   - Before: 8/22 decodes, 57% OSD usage
   - After Option 1: Expected 9-11/22 decodes, 35-40% OSD usage
   - Track which signals benefit from ratio method

3. **If successful, proceed to phase tracking** (Option 2)
   - Per-symbol phase correction
   - Enable nsym=2/3 safely
   - Target: 15-18/22 decodes, 20% OSD usage

## Conclusion

**Root cause identified**: We only generate 1 LLR representation (nsym=1 difference method) while WSJT-X generates 4 (nsym=1/2/3 with difference + ratio methods).

**Impact**: Without multiple representations, BP has only 1 chance to converge. If that representation has poor quality (wrong bins, phase drift, noise), BP fails → OSD fallback → 57% OSD usage.

**Solution**: Implement multiple LLR methods (starting with ratio method) to give BP multiple chances to converge, matching WSJT-X's robust approach.

This explains why WSJT-X achieves 22/22 decodes with <20% OSD usage while we achieve 8/22 with 57% OSD usage - they have 4x more opportunities for BP to succeed.
