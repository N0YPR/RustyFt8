# Dual LLR Implementation - 2025-11-25

## Objective

Implement WSJT-X's dual LLR method strategy (difference + ratio methods) to improve BP convergence and reduce OSD usage from 57% to <20%.

## Background

From [llr_4pass_discovery_20251125.md](llr_4pass_discovery_20251125.md):
- WSJT-X uses 4 LLR representations per signal (llra, llrb, llrc, llrd)
- llra = difference method for nsym=1
- llrb = difference method for nsym=2
- llrc = difference method for nsym=3
- llrd = ratio method for nsym=1

**Key insight**: We only generated 1 LLR representation while WSJT-X generates 4, giving BP 4x more chances to converge.

## Implementation

### Phase 1: Ratio Method LLR (Completed)

Implemented dual LLR extraction computing both difference and ratio methods in a single pass.

**Files Modified**:

#### src/sync/extract.rs

**Lines 121-128**: Updated extract_symbols_impl signature
```rust
fn extract_symbols_impl(
    signal: &[f32],
    candidate: &Candidate,
    nsym: usize,
    llr: &mut [f32],
    mut llr_ratio_out: Option<&mut [f32]>,  // Optional ratio LLR output
    s8_out: Option<&mut [[f32; 79]; 8]>,
) -> Result<(), String> {
```

**Lines 529-540, 593-604, 641-652, 706-717**: Added ratio LLR computation at all 4 nsym branches
```rust
// Standard difference method LLR (current method)
llr[bit_idx] = max_mag_1 - max_mag_0;

// Ratio method LLR (WSJT-X llrd equivalent)
if let Some(ref mut llr_ratio) = llr_ratio_out {
    let den = max_mag_1.max(max_mag_0);
    llr_ratio[bit_idx] = if den > 0.0 {
        (max_mag_1 - max_mag_0) / den
    } else {
        0.0
    };
}
```

**Lines 769-796**: Added normalization for ratio LLRs
```rust
// Normalize ratio method LLRs (if provided)
if let Some(ref mut llr_ratio) = llr_ratio_out {
    let mut sum_r = 0.0f32;
    let mut sum_sq_r = 0.0f32;
    for i in 0..174 {
        sum_r += llr_ratio[i];
        sum_sq_r += llr_ratio[i] * llr_ratio[i];
    }
    let mean_r = sum_r / 174.0;
    let mean_sq_r = sum_sq_r / 174.0;
    let variance_r = mean_sq_r - mean_r * mean_r;
    let std_dev_r = if variance_r > 0.0 {
        variance_r.sqrt()
    } else {
        mean_sq_r.sqrt()
    };

    if std_dev_r > 0.0 {
        for i in 0..174 {
            llr_ratio[i] /= std_dev_r;
        }
    }

    // Then scale by WSJT-X scalefac=2.83
    for i in 0..174 {
        llr_ratio[i] *= 2.83;
    }
}
```

**Lines 840-856**: Added public dual LLR extraction function
```rust
/// Extract symbols with DUAL LLR methods (difference and ratio)
///
/// Computes both standard difference LLR (max_1 - max_0) and normalized ratio LLR
/// ((max_1 - max_0) / max(max_1, max_0)) in a single pass. This matches WSJT-X's
/// 4-pass strategy where llra uses difference method and llrd uses ratio method.
///
/// The ratio method provides a normalized LLR that's more robust to amplitude variations.
pub fn extract_symbols_dual_llr(
    signal: &[f32],
    candidate: &Candidate,
    nsym: usize,
    llr_diff: &mut [f32],
    llr_ratio: &mut [f32],
    s8_out: &mut [[f32; 79]; 8],
) -> Result<(), String> {
    extract_symbols_impl(signal, candidate, nsym, llr_diff, Some(llr_ratio), Some(s8_out))
}
```

#### src/sync/mod.rs

**Line 39**: Exported new dual LLR function
```rust
pub use extract::{extract_symbols, extract_symbols_with_powers, extract_symbols_dual_llr, calculate_snr};
```

#### src/decoder.rs

**Lines 124-144**: Updated to use dual LLR extraction
```rust
// Try multi-pass decoding with dual LLR methods (matching WSJT-X 4-pass strategy)
for &nsym in &nsym_values {
    let mut llr_diff = vec![0.0f32; 174];   // Difference method (llra)
    let mut llr_ratio = vec![0.0f32; 174];  // Ratio method (llrd)
    let mut s8 = [[0.0f32; 79]; 8];

    // Extract symbols with BOTH LLR methods in one pass
    let extract_ok = sync::extract_symbols_dual_llr(
        signal, &refined, nsym, &mut llr_diff, &mut llr_ratio, &mut s8
    ).is_ok();

    if !extract_ok {
        continue;
    }

    // Try both LLR methods (difference and ratio) with multiple scales
    // This matches WSJT-X's pass 1 (llra) and pass 4 (llrd)
    let llr_methods: [(&str, &[f32]); 2] = [
        ("diff", &llr_diff[..]),
        ("ratio", &llr_ratio[..]),
    ];

    for &(method_name, llr) in &llr_methods {
```

**Lines 186-196**: Updated debug output to show which method decoded
```rust
// Debug: log LDPC decoder type and LLR method (disabled by default)
let _debug_ldpc = false;
if _debug_ldpc {
    let decode_type = if iters == 0 {
        "OSD"
    } else {
        "BP"
    };
    eprintln!("  LDPC: {} iters={}, method={}, freq={:.1} Hz, nsym={}, scale={:.1}",
             decode_type, iters, method_name, refined.frequency, nsym, scale);
}
```

## Algorithm Comparison

### WSJT-X ft8b.f90 (lines 211-225)

**nsym=1 generates TWO LLR arrays**:

```fortran
if(nsym.eq.1) then
  bmeta(i32+ib)=bm              ! Difference: max(ones) - max(zeros)

  den=max(maxval(s2(0:nt-1),one(0:nt-1,ibmax-ib)), &
          maxval(s2(0:nt-1),.not.one(0:nt-1,ibmax-ib)))
  if(den.gt.0.0) then
    cm=bm/den                    ! Ratio: (max(ones) - max(zeros)) / max(max(ones), max(zeros))
  else
    cm=0.0
  endif
  bmetd(i32+ib)=cm              ! Store ratio variant
```

Then all 4 arrays (bmeta, bmetb, bmetc, bmetd) are:
1. Normalized by standard deviation (normalizebmet)
2. Scaled by 2.83
3. Tried sequentially for BP decoding (lines 265-269)

### RustyFt8 Implementation

**Before**: Only difference method
```rust
llr[bit_idx] = max_mag_1 - max_mag_0;  // Single representation
```

**After**: Both difference and ratio methods
```rust
// Difference method
llr[bit_idx] = max_mag_1 - max_mag_0;

// Ratio method (if requested)
if let Some(ref mut llr_ratio) = llr_ratio_out {
    let den = max_mag_1.max(max_mag_0);
    llr_ratio[bit_idx] = if den > 0.0 {
        (max_mag_1 - max_mag_0) / den
    } else {
        0.0
    };
}
```

Then BOTH arrays normalized and scaled identically to WSJT-X.

## Why This Helps

### Difference Method (max_1 - max_0)
- **Raw magnitude difference** between "1" and "0" hypotheses
- Works well when signal has consistent amplitude
- Sensitive to amplitude variations across symbols

### Ratio Method ((max_1 - max_0) / max(max_1, max_0))
- **Normalized by signal strength** at each bit
- More robust to amplitude fading/variations
- Provides independent "view" of bit confidence
- Can have better LLR quality when signal has amplitude variations

### Combined Strategy

For each candidate, BP tries:
1. Difference LLR with scales [1.0, 1.5, 0.75, ...]
2. Ratio LLR with scales [1.0, 1.5, 0.75, ...]

This gives **2x more opportunities** for BP to converge compared to single method.

If difference method has poor quality (e.g., amplitude variations), ratio method may work. And vice versa.

## Expected Results

### Before (Single Method)
- Total: 8/22 decodes (36%)
- OSD usage: 57% (4/7 Pass 1 decodes)
- Strong signals (-4 to -8 dB) use OSD

### After (Dual Method - Expected)
- Total: 9-11/22 decodes (41-50%)
- OSD usage: 35-40% (target: eventual <20%)
- Some strong signals switch from OSD to BP
- 1-3 new decodes from ratio method working where difference failed

### Ultimate Goal (with nsym=2/3)
- Total: 15-18/22 decodes (68-82%)
- OSD usage: ~20%
- After fixing phase tracking, enable nsym=2/3 for full 4-pass strategy
- Expected to match WSJT-X: 22/22 (100%), <20% OSD usage

## Testing

### Test Command
```bash
cargo test --test real_ft8_recording 2>&1 | grep -E "(Pass [0-9]|Total|message:)" | head -60
```

### Metrics to Track
1. **Total decodes**: 8/22 → ?/22
2. **OSD usage**: 57% → ?%
3. **Method effectiveness**: How many use "diff" vs "ratio"
4. **BP convergence**: Do strong signals now use BP?

### Enable Debug Output
To see which method decoded each signal:
```rust
let _debug_ldpc = true;  // In decoder.rs line 187
```

Output will show:
```
LDPC: BP iters=5, method=diff, freq=2572.0 Hz, nsym=1, scale=1.0
LDPC: BP iters=8, method=ratio, freq=2157.0 Hz, nsym=1, scale=1.5
LDPC: OSD iters=0, method=diff, freq=642.0 Hz, nsym=1, scale=2.0
```

## Next Steps

### If Successful (OSD usage 35-40%)
1. Document which signals benefit from ratio method
2. Proceed to Phase 2: Fix phase tracking for nsym=2/3
3. Implement full 4-pass strategy (llra, llrb, llrc, llrd)
4. Target: 22/22 decodes, <20% OSD usage

### If Marginal (OSD usage 50-55%)
1. Investigate why ratio method not helping
2. Check if ratio LLR quality is good (mean, max values)
3. May need to adjust normalization or scaling
4. Consider other LLR quality improvements first

### If No Improvement (OSD usage still 57%)
1. Verify ratio LLRs are being computed correctly
2. Check if BP is actually trying ratio method
3. May indicate deeper LLR quality issues (e.g., tone extraction errors)
4. Revisit tone extraction and in-band interference handling

## Code Quality

**Compilation**: ✅ Successful with warnings only
**Runtime**: Pending test results
**Backward compatibility**: ✅ Old `extract_symbols` still works
**Performance**: Minimal impact (single-pass computation of both methods)

## Comparison with WSJT-X

| Feature | WSJT-X | RustyFt8 Before | RustyFt8 After |
|---------|--------|-----------------|----------------|
| **LLR methods** | 2 (diff + ratio) | 1 (diff only) | 2 (diff + ratio) ✅ |
| **nsym values** | 1, 2, 3 | 1 only | 1 only |
| **Total LLR arrays** | 4 (llra, llrb, llrc, llrd) | 1 | 2 (llra, llrd) |
| **BP attempts per candidate** | ~64 (4 LLR × 16 scales) | ~16 (1 LLR × 16 scales) | ~32 (2 LLR × 16 scales) |
| **Expected OSD usage** | <20% | 57% | 35-40% → <20% |

## References

- [llr_4pass_discovery_20251125.md](llr_4pass_discovery_20251125.md): Root cause analysis
- [session_20251125_part3.md](session_20251125_part3.md): OSD dominance discovery
- WSJT-X ft8b.f90 lines 211-225: Dual LLR computation
- WSJT-X ft8b.f90 lines 265-269: 4-pass decode loop

## Conclusion

Implemented first phase of WSJT-X's 4-pass strategy: dual LLR methods (difference + ratio) for nsym=1. This doubles the number of BP attempts per candidate, giving BP more chances to converge before falling back to OSD.

**Status**: Code complete, awaiting test results to validate improvement.

Next phase will add nsym=2/3 support (requires phase tracking fixes) to complete the full 4-pass strategy and achieve WSJT-X parity.
