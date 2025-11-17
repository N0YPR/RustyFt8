# WSJT-X Decoder Strategy Analysis

## 2025-11-17 - Source Code Analysis

### Key Discovery

After comparing WSJT-X's `jt9` output (22 decodes) with RustyFt8 (5 decodes), analyzed their source code to understand the gap.

### WSJT-X's Hybrid BP/OSD Strategy

From `wsjtx-2.7.0/src/wsjtx/lib/ft8/decode174_91.f90`:

```fortran
! maxosd<0: do bp only
! maxosd=0: do bp and then call osd once with channel llrs
! maxosd>1: do bp and then call osd maxosd times with saved bp outputs
! norder  : osd decoding depth
```

**Implementation (lines 52-148):**
1. Run BP for up to 30 iterations
2. **Save LLRs from iterations 1, 2, 3** (lines 62-64)
3. If BP converges with good CRC, return (line 87)
4. After BP exhausts, loop through saved iterations (lines 137-148):
   - Try OSD with LLRs from iteration 1
   - If fails, try OSD with LLRs from iteration 2
   - If fails, try OSD with LLRs from iteration 3

**Parameters (from ft8b.f90 lines 405-412):**

```fortran
norder=2        ! OSD order 2
maxosd=2        ! Try OSD 2 times

if(ndepth.eq.1) maxosd=-1  ! Depth 1: BP only
if(ndepth.eq.2) maxosd=0   ! Depth 2: BP + OSD once (with channel LLRs)
if(ndepth.eq.3...) maxosd=2 ! Depth 3: BP + OSD twice (with BP iters 1, 2)
```

### RustyFt8's Current Strategy

From `src/ldpc/mod.rs`:

```rust
// Try Belief Propagation first
if let Some((decoded, iters)) = decode(llr, 200) {
    return Some((decoded, iters));
}

// If BP fails, try OSD order 4
if let Some(decoded) = osd_decode(llr, 4) {
    return Some((decoded, 0));
}
```

**Key Differences:**
1. We try OSD **only once** with final LLRs
2. WSJT-X tries OSD **multiple times** with intermediate BP results
3. We use OSD order **4** vs WSJT-X's order **2**
4. We run BP for **200** iterations vs WSJT-X's **30**

### Why Multiple OSD Attempts Matter

**Theory:** Early BP iterations have different error characteristics:
- **Iteration 1:** Closer to channel LLRs, less correlated errors
- **Iteration 2:** Partially converged, different error pattern
- **Final iteration:** May have converged to wrong codeword neighborhood

By trying OSD with **multiple snapshots**, WSJT-X explores different regions of the solution space.

### A Priori Decoding

WSJT-X also uses A Priori (AP) decoding with known message patterns:

From `ft8b.f90` lines 60-81:
```fortran
nappasses(0)=2  ! 2 passes when transmitting CQ
nappasses(1)=2  ! 2 passes after receiving CQ response
...
nappasses(3)=4  ! 4 passes during QSO
nappasses(4)=4

! iaptype meanings:
!   1  CQ     ???    ???     (32 AP bits)
!   2  MyCall ???    ???     (32 AP bits)
!   3  MyCall DxCall ???     (61 AP bits)
!   4  MyCall DxCall RRR     (77 AP bits)
!   5  MyCall DxCall 73      (77 AP bits)
!   6  MyCall DxCall RR73    (77 AP bits)
```

For each decode attempt, WSJT-X tries multiple AP patterns depending on QSO state.

### Test Results Comparison

**Test file:** `210703_133430.wav`

| Decoder | Decodes | Weakest SNR | Strategy |
|---------|---------|-------------|----------|
| WSJT-X | 22 | -24 dB | BP(30) + OSD(2)×2 + AP |
| RustyFt8 | 5 | ~-16 dB | BP(200) + OSD(4)×1 |
| **Gap** | **17** | **8 dB** | |

**WSJT-X decoded signals we missed:**
- -24 dB: TU; 7N9RST EI8TRF 589 5732
- -20 dB: K1JT HA5WA 73 (at 2039 Hz)
- -20 dB: K1BZM DK8NE -10 (at 244 Hz)
- -20 dB: CQ EA2BFM IN83 (at 2280 Hz)
- -17 dB: N1API F2VX 73 (at 1513 Hz)
- -17 dB: CQ DX DL8YHR JO41 (at 2606 Hz)
- Plus many others from -16 to -7 dB

### Multi-Pass Subtraction Context

**Previous hypothesis:** Poor signal subtraction (-0.4 dB) was blocking multi-pass gains.

**Reality:** Even with perfect subtraction, we'd only be finding strong signals. WSJT-X gets its advantage from:
1. **Better weak signal decoding** (multiple OSD + AP)
2. Signal subtraction allowing pass 2/3 (but less critical than decoder quality)

### Recommended Implementation

**Priority 1: Multiple OSD Attempts** ✅ **IMPLEMENTED - 2025-11-17**

**Result:** **25 decodes** vs WSJT-X's 22 (118% of target, 5x improvement over previous 5 decodes)

See [HYBRID_DECODER_RESULTS.md](HYBRID_DECODER_RESULTS.md) for complete implementation details and test results.

**Original Plan:**

Implement WSJT-X's strategy:

```rust
pub fn decode_with_saved_iters(llr: &[f32], max_bp_iters: usize) -> Option<(BitVec<u8, Msb0>, usize)> {
    let mut saved_llrs = Vec::new();

    // Run BP and save intermediate LLRs
    let result = decode_with_snapshots(llr, max_bp_iters, &mut saved_llrs, &[1, 2, 10]);

    if let Some(decoded) = result {
        return Some(decoded);
    }

    // Try OSD with each saved snapshot
    for (iter, llr_snapshot) in saved_llrs.iter().enumerate() {
        if let Some(decoded) = osd_decode(llr_snapshot, 2) {
            eprintln!("OSD succeeded with iteration {} LLRs", iter + 1);
            return Some((decoded, 0));
        }
    }

    None
}
```

**Expected gain:** 8-12 additional decodes (closing ~50-70% of gap)

**Priority 2: A Priori Decoding** (Medium Impact)

Implement message pattern hints:
- CQ patterns
- Callsign patterns (when known)
- Standard QSO exchanges (RRR, 73, RR73)

**Expected gain:** 3-5 additional decodes (closing remaining gap)

**Priority 3: LLR Normalization** (Small Impact)

Fine-tune LLR scaling based on SNR estimates.

**Expected gain:** 1-2 additional decodes

### Conclusion

The performance gap is primarily due to **decoder sophistication**, not signal subtraction:

| Component | Impact on Gap |
|-----------|---------------|
| Multiple OSD attempts | **HIGH** (50-70%) |
| A Priori decoding | **MEDIUM** (20-30%) |
| Signal subtraction | **LOW** (10-20%) |
| LLR normalization | **MINIMAL** (5-10%) |

Signal subtraction investigation was valuable but revealed that decoder improvements are the critical path to matching WSJT-X performance.

### References

- WSJT-X source: `wsjtx-2.7.0/src/wsjtx/lib/ft8/decode174_91.f90`
- WSJT-X decoder: `wsjtx-2.7.0/src/wsjtx/lib/ft8/ft8b.f90`
- Test file: `tests/test_data/210703_133430.wav`
- WSJT-X output: 22 decodes with SNR range +16 to -24 dB
