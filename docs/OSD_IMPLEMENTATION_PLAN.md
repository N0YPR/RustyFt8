# OSD (Ordered Statistics Decoding) Implementation Plan

## Current Status

**Problem:** Simplified bit-flip OSD recovered 0/44 failed candidates
**Root Cause:** Doesn't respect LDPC code structure - needs generator matrix transformation

## What is Proper OSD?

OSD exploits the fact that some received bits are more reliable than others. It:
1. **Orders bits by reliability** (|LLR| magnitude)
2. **Performs Gaussian elimination** on the generator matrix in GF(2)
3. **Systematically tests information patterns** using the transformed generator
4. **Finds valid codewords** that pass CRC check

The key insight: After reordering and Gaussian elimination, the K most reliable bits can be treated as "information bits" and the rest are derived from them via the parity check constraints.

## Algorithm Breakdown

### Phase 1: Setup (One-Time, Can Be Cached)

#### 1.1 Generate the Generator Matrix
```rust
// Create 91×174 matrix where each row encodes a unit message
// Row i = encode([0,0,...,1,...,0,0]) with 1 at position i
fn generate_generator_matrix() -> [[bool; N]; K] {
    let mut gen = [[false; N]; K];
    for i in 0..K {
        let mut msg = [false; K];
        msg[i] = true;
        gen[i] = ldpc::encode(&msg);
    }
    gen
}
```

**Complexity:** O(K × N × M) where M is LDPC encoding cost
**One-time cost:** ~91ms (can cache this matrix)

#### 1.2 The Generator Matrix Structure

For FT8 LDPC(174,91):
```
Generator = [K rows × N columns] = [91 × 174]

Each row represents encoding of a single information bit:
Row 0: encode([1,0,0,...,0])  →  174-bit codeword
Row 1: encode([0,1,0,...,0])  →  174-bit codeword
...
Row 90: encode([0,0,...,0,1]) →  174-bit codeword
```

### Phase 2: Per-Candidate Decoding

#### 2.1 Order Bits by Reliability
```rust
// Sort bit indices by |LLR| (most reliable first)
let mut indices: Vec<usize> = (0..N).collect();
indices.sort_by(|&a, &b| {
    llr[b].abs().partial_cmp(&llr[a].abs()).unwrap()
});
```

**Complexity:** O(N log N) = O(174 log 174) ≈ 1.3K comparisons

#### 2.2 Reorder Generator Matrix Columns
```rust
// Permute columns to match bit reliability ordering
fn reorder_generator(gen: &[[bool; N]; K], indices: &[usize]) -> [[bool; N]; K] {
    let mut reordered = [[false; N]; K];
    for i in 0..K {
        for j in 0..N {
            reordered[i][j] = gen[i][indices[j]];
        }
    }
    reordered
}
```

**Complexity:** O(K × N) = O(91 × 174) ≈ 15.8K operations

#### 2.3 Gaussian Elimination in GF(2)

This is the **critical step** that my current implementation is missing!

```rust
// Convert generator matrix to reduced row echelon form (RREF)
// Goal: Get identity matrix in the first K columns (most reliable bits)
fn gaussian_elimination_gf2(matrix: &mut [[bool; N]; K], indices: &mut [usize]) {
    for diag in 0..K {
        // Find pivot: look for a 1 in column 'diag' or nearby columns
        let mut pivot_col = None;
        for col in diag..(diag + 20).min(N) {
            if matrix[diag][col] {
                pivot_col = Some(col);
                break;
            }
        }

        let pivot_col = match pivot_col {
            Some(col) => col,
            None => continue, // Degenerate case - skip
        };

        // Swap columns if needed (also swap in indices array)
        if pivot_col != diag {
            for row in 0..K {
                matrix[row].swap(diag, pivot_col);
            }
            indices.swap(diag, pivot_col);
        }

        // Eliminate: XOR all rows that have 1 in column 'diag'
        for row in 0..K {
            if row != diag && matrix[row][diag] {
                // XOR row with pivot row (GF(2) addition)
                for col in 0..N {
                    matrix[row][col] ^= matrix[diag][col];
                }
            }
        }
    }
}
```

**Result:** After Gaussian elimination:
```
First K columns ≈ Identity matrix (as close as possible)
Last (N-K) columns = parity generator

Matrix now looks like:
[ 1 0 0 ... 0 | p p p ... p ]  Row 0
[ 0 1 0 ... 0 | p p p ... p ]  Row 1
[ 0 0 1 ... 0 | p p p ... p ]  Row 2
    ...
[ 0 0 0 ... 1 | p p p ... p ]  Row K-1

Where 'p' are parity bits determined by information bits
```

**Complexity:** O(K² × N) = O(91² × 174) ≈ 1.4M XOR operations
**Actual time:** ~5-10ms per candidate

#### 2.4 Fast Encoding with Transformed Generator

Now we can encode any K-bit information pattern to N-bit codeword:

```rust
// Encode using the RREF generator matrix
fn encode_with_rref(info_bits: &[bool; K], gen_rref: &[[bool; N]; K]) -> [bool; N] {
    let mut codeword = [false; N];

    // For each information bit that is 1, XOR the corresponding generator row
    for i in 0..K {
        if info_bits[i] {
            for j in 0..N {
                codeword[j] ^= gen_rref[i][j];
            }
        }
    }

    codeword
}
```

**Complexity:** O(K × N) in worst case, but sparse - typically O(K × k) where k ≈ 30-50
**Actual time:** ~2-5μs per encoding

#### 2.5 Order-0 Decoding

```rust
// Make hard decisions on reordered bits
let mut hard_dec = [false; N];
for i in 0..N {
    hard_dec[i] = llr[indices[i]] >= 0.0;
}

// Take first K bits (most reliable) as information
let info_bits = &hard_dec[0..K];

// Encode to get candidate codeword
let candidate = encode_with_rref(info_bits, &gen_rref);

// Un-permute back to original bit order
let mut unpermuted = [false; N];
for i in 0..N {
    unpermuted[indices[i]] = candidate[i];
}

// Extract message and check CRC
let msg91 = &unpermuted[0..91];
if crc14_check(msg91) {
    return Some(msg91);
}
```

**Complexity:** O(K × N) ≈ 15K operations
**Expected recovery:** 5-10 additional signals (order-0 alone)

#### 2.6 Order-1 Decoding (Single Bit Flips)

```rust
// If order-0 failed, try flipping each of the least reliable K bits
let flip_width = 40; // Test 40 least reliable positions

for flip_pos in (K - flip_width)..K {
    let mut test_info = info_bits.clone();
    test_info[flip_pos] = !test_info[flip_pos];

    let candidate = encode_with_rref(&test_info, &gen_rref);

    // Un-permute and check
    // ... (same as order-0)

    if crc14_check(msg91) {
        return Some(msg91);
    }
}
```

**Complexity:** O(flip_width × K × N) = O(40 × 91 × 174) ≈ 632K operations
**Expected recovery:** 3-5 additional signals
**Actual time:** ~10-20ms per candidate

#### 2.7 Order-2 Decoding (Bit Pairs)

```rust
// Try flipping pairs of unreliable bits
let flip_width = 20; // Smaller due to O(n²) explosion

for i in 0..flip_width {
    for j in (i+1)..flip_width {
        let pos_i = K - flip_width + i;
        let pos_j = K - flip_width + j;

        let mut test_info = info_bits.clone();
        test_info[pos_i] = !test_info[pos_i];
        test_info[pos_j] = !test_info[pos_j];

        let candidate = encode_with_rref(&test_info, &gen_rref);

        // Un-permute and check...
    }
}
```

**Complexity:** O(flip_width² × K × N / 2) ≈ 3.5M operations
**Expected recovery:** 1-3 additional signals
**Actual time:** ~50-100ms per candidate

### Phase 3: Distance-Based Selection (Advanced)

WSJT-X also computes Euclidean distance to select best candidate:

```rust
fn euclidean_distance(codeword: &[bool; N], llr: &[f32], indices: &[usize]) -> f32 {
    let mut dist = 0.0;
    for i in 0..N {
        let orig_idx = indices[i];
        let hard_dec = llr[orig_idx] >= 0.0;
        if codeword[i] != hard_dec {
            dist += llr[orig_idx].abs();
        }
    }
    dist
}
```

Track minimum distance and return the best candidate even if multiple pass CRC.

## Implementation Strategy

### Step 1: Core Infrastructure (Day 1)
1. Implement GF(2) Gaussian elimination
2. Generator matrix generation and caching
3. Fast encoding with RREF matrix
4. Unit tests for each component

**Files:**
- `src/ldpc/osd.rs` - Main OSD implementation
- `src/ldpc/generator.rs` - Generator matrix utilities
- `src/ldpc/gf2.rs` - GF(2) arithmetic and Gaussian elimination

### Step 2: Order-0 Implementation (Day 1-2)
1. Bit ordering by |LLR|
2. Matrix reordering
3. Gaussian elimination
4. Order-0 decode with CRC check
5. Un-permutation

**Expected impact:** +5-10 decodes (6 → 11-16)

### Step 3: Order-1 Implementation (Day 2)
1. Systematic single-bit flip testing
2. Configurable flip width
3. Early exit on success

**Expected impact:** +3-5 decodes (11-16 → 14-21)

### Step 4: Order-2 Implementation (Day 2-3)
1. Bit-pair flip testing
2. Configurable flip width (smaller due to O(n²))
3. Distance-based selection

**Expected impact:** +1-3 decodes (14-21 → 15-22+)

### Step 5: Optimization (Day 3)
1. Cache generator matrix (one-time compute)
2. Parallelize order-1/2 searches with rayon
3. Early exit strategies
4. Profile and optimize hot paths

**Expected speedup:** 2-3x

## Performance Targets

### Per-Candidate Costs (Estimated)

| Operation | Complexity | Time | Cumulative |
|-----------|------------|------|------------|
| Generate generator (cached) | O(K×N×M) | 91ms | One-time |
| Bit ordering | O(N log N) | 0.01ms | 0.01ms |
| Matrix reordering | O(K×N) | 0.02ms | 0.03ms |
| Gaussian elimination | O(K²×N) | 8ms | 8.03ms |
| Order-0 test | O(K×N) | 0.02ms | 8.05ms |
| Order-1 (40 flips) | O(40×K×N) | 15ms | 23ms |
| Order-2 (190 pairs) | O(190×K×N) | 70ms | 93ms |

### Full Decode Session (50 candidates)

| Configuration | Time per Candidate | Total Time | Expected Decodes |
|--------------|-------------------|------------|------------------|
| BP only | 20ms | 1.0s | 6 |
| BP + OSD-0 | 28ms | 1.4s | 11-16 |
| BP + OSD-1 | 43ms | 2.15s | 14-21 |
| BP + OSD-2 | 113ms | 5.65s | 15-22+ |

**Target:** BP + OSD-1 as default (2.15s total, 14-21 decodes)

## Testing Strategy

### Unit Tests
```rust
#[test]
fn test_gf2_gaussian_elimination() {
    // Test with known matrices
}

#[test]
fn test_generator_matrix_properties() {
    // Verify matrix rank, dimensions
}

#[test]
fn test_encode_decode_roundtrip() {
    // Ensure encoding is correct
}

#[test]
fn test_osd_order0_perfect_signal() {
    // Should decode clean signal
}

#[test]
fn test_osd_order1_single_error() {
    // Should correct 1-bit error
}
```

### Integration Tests
```rust
#[test]
fn test_osd_on_real_recording() {
    // Measure improvement: 6 → 14-21
}

#[test]
fn test_osd_vs_wsjt_agreement() {
    // Validate all decoded messages
}

#[test]
fn test_osd_performance() {
    // Ensure < 5s total decode time
}
```

## Expected Results

### Conservative Estimate
- Order-0: +5 decodes → 11 total (50% of WSJT-X)
- Order-1: +3 decodes → 14 total (64% of WSJT-X)
- Order-2: +1 decode → 15 total (68% of WSJT-X)

### Optimistic Estimate
- Order-0: +10 decodes → 16 total (73% of WSJT-X)
- Order-1: +5 decodes → 21 total (95% of WSJT-X)
- Order-2: +2 decodes → 22+ total (100%+ of WSJT-X)

### Remaining Gap (if any)
After full OSD implementation, remaining differences likely due to:
1. **A Priori decoding** - Uses QSO context (callsign, grid) to hint decoder
2. **Signal subtraction** - Removes decoded signals, reveals masked weaker ones
3. **Multi-pass strategies** - WSJT-X makes 4-6 passes with different parameters

## Risks and Mitigations

### Risk 1: Gaussian Elimination Bugs
**Impact:** OSD won't work at all
**Mitigation:**
- Extensive unit tests with known matrices
- Compare intermediate results with WSJT-X
- Start with small test cases

### Risk 2: Performance Too Slow
**Impact:** Decode time > 10 seconds (unacceptable)
**Mitigation:**
- Profile early and often
- Parallelize order-1/2 with rayon
- Early exit on first valid decode
- Limit search space intelligently

### Risk 3: Still No Improvement
**Impact:** Same 6 decodes despite proper OSD
**Mitigation:**
- Validate against WSJT-X step-by-step
- Check that we're using same LLRs
- Verify CRC check is correct
- Compare generator matrices

## Success Criteria

✅ **Minimum Success:** 11+ decodes (5 additional from OSD)
✅ **Target Success:** 15+ decodes (9 additional from OSD)
✅ **Stretch Goal:** 20+ decodes (14 additional from OSD)

All decoded messages must match WSJT-X reference output.

## Next Steps

1. **Review this plan** - Confirm approach makes sense
2. **Start with Step 1** - Core infrastructure (Gaussian elimination)
3. **Implement incrementally** - Test after each step
4. **Measure continuously** - Track decode count improvements

Estimated total effort: **2-3 days** for full BP + OSD-1 implementation.
