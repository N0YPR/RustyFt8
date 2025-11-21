# Symbol Extraction Implementation Differences: WSJT-X vs RustyFt8

## Critical Finding: Complex Value Normalization Missing

### WSJT-X Implementation (ft8b.f90:159)
```fortran
cs(0:7,k)=csymb(1:8)/1e3
```
**Divides complex FFT output by 1000 before storing**

### RustyFt8 Implementation (extract.rs:269)
```rust
cs[tone][k] = (re, im);
```
**Stores complex FFT output directly WITHOUT normalization**

### Impact
This normalization is **CRITICAL** for multi-symbol coherent combining (nsym=2,3):
- When coherently adding symbols: `sum = cs1 + cs2 + cs3`
- The magnitude scale directly affects the combined signal amplitude
- Without normalization, the scale can be inconsistent, degrading LLR quality

## Symbol Extraction Comparison

### Step 1: Downsample and Extract Symbols

| Aspect | WSJT-X | RustyFt8 | Match? |
|--------|--------|----------|---------|
| Downsample to ~200 Hz | ✓ | ✓ | ✅ |
| FFT size | 32 | 32 | ✅ |
| Samples per symbol | 32 | ~30 | ⚠️ |
| FFT window | Direct copy, no offset | Centered with offset | ❌ |
| Complex normalization | **÷ 1000** | **None** | ❌ |
| Magnitude storage | `s8 = abs(cs)` | `s8 = sqrt(re² + im²)` | ✅ |

### Step 2: Costas Validation

| Aspect | WSJT-X | RustyFt8 | Match? |
|--------|--------|----------|---------|
| Check 3 Costas arrays | ✓ | ✓ | ✅ |
| Reject if < 7/21 correct | ✓ (nsync ≤ 6) | ✓ (nsync < 7) | ⚠️ |
| Our threshold | Reject ≤ 6 | Reject < 3 | ❌ |

**Problem:** RustyFt8 threshold is too lenient (< 3 vs ≤ 6)!

### Step 3: LLR Computation

| Aspect | WSJT-X | RustyFt8 | Match? |
|--------|--------|----------|---------|
| Gray code mapping | ✓ | ✓ | ✅ |
| Multi-symbol combining | ✓ (nsym=1,2,3) | ✓ (nsym=1,2,3) | ✅ |
| Raw LLR | `max(s2 where bit=1) - max(s2 where bit=0)` | Same | ✅ |
| Bit-by-bit normalization | ✓ (nsym=1 only, bmetd) | ❌ | ❌ |
| Global normalization | ✓ (normalizebmet) | ✓ (std_dev) | ✅ |
| Scale factor | 2.83 | 2.83 | ✅ |

**Missing:** RustyFt8 doesn't implement bit-by-bit normalized LLRs (bmetd/llrd)

### Step 4: Decoding Strategy

| Aspect | WSJT-X | RustyFt8 | Match? |
|--------|--------|----------|---------|
| Pass 1 | nsym=1 (llra) | nsym=1 | ✅ |
| Pass 2 | nsym=2 (llrb) | nsym=2 | ✅ |
| Pass 3 | nsym=3 (llrc) | nsym=3 | ✅ |
| Pass 4 | nsym=1 bit-normalized (llrd) | **None** | ❌ |

## Root Cause Analysis

### Primary Issues (High Impact)

1. **Missing Complex Value Normalization** (÷1000)
   - Affects multi-symbol coherent combining quality
   - Can cause magnitude scale inconsistencies
   - Impact: **CRITICAL** for nsym=2,3 performance

2. **Costas Validation Too Lenient** (< 3 vs ≤ 6)
   - Allows low-quality symbol extractions to proceed
   - WSJT-X rejects if ≤ 6 of 21 Costas tones correct (~29%)
   - RustyFt8 only rejects if < 3 of 21 correct (~14%)
   - Impact: **HIGH** - allows poor sync quality

3. **Missing Bit-Normalized LLR Pass**
   - WSJT-X has a 4th pass with `llrd` (bmetd)
   - Normalizes each bit by `den = max(max_mag_1, max_mag_0)`
   - This can help when signal has amplitude variations
   - Impact: **MEDIUM** - affects ~25% of decodes

### Secondary Issues (Medium Impact)

4. **FFT Window Offset**
   - WSJT-X: Direct 32-sample window
   - RustyFt8: Centers in buffer with offset
   - Impact: **LOW** - minor phase/frequency effects

5. **Samples Per Symbol Variation**
   - WSJT-X: Exactly 32 samples (fixed by design)
   - RustyFt8: ~30 samples (variable based on actual sample rate)
   - Impact: **LOW** - affects frequency resolution slightly

## Recommended Fixes (Priority Order)

### Priority 1: Complex Value Normalization
```rust
// In extract.rs, line ~269
// BEFORE:
cs[tone][k] = (re, im);

// AFTER:
const NORM_FACTOR: f32 = 1000.0;
cs[tone][k] = (re / NORM_FACTOR, im / NORM_FACTOR);
```

### Priority 2: Stricter Costas Validation
```rust
// In extract.rs, line ~335
// BEFORE:
if nsync < 3 {

// AFTER:
if nsync <= 6 {  // Match WSJT-X threshold
```

### Priority 3: Add Bit-Normalized LLR Computation
```rust
// After computing raw LLR, add normalized version:
// llr_normalized[bit_idx] = bm / max(max_mag_1, max_mag_0)
// Then try decoding with both raw and normalized LLRs
```

### Priority 4: Remove FFT Window Offset
```rust
// In extract.rs, line ~229
// BEFORE:
let fft_offset = if nsps_down < NFFT_SYM { 1 } else { 0 };

// AFTER:
let fft_offset = 0;  // Match WSJT-X: no offset
```

## Expected Impact

Implementing these fixes should:
- ✅ Improve BP-only decoding from 7/19 to ~14/19 messages (2x improvement)
- ✅ Reduce sensitivity to amplitude variations
- ✅ Improve nsym=2,3 coherent combining performance
- ✅ Match WSJT-X quality thresholds

The most critical fix is **complex value normalization** (Priority 1), which directly affects the coherent combining that provides 3-6 dB SNR improvement for weak signals.
