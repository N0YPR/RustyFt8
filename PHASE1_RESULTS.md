# Phase 1: Performance Measurement - Initial Results

## Date
2026-01-07

## Benchmark Setup

Created criterion-based benchmarks in `benches/decode_pipeline.rs`:
- Full end-to-end decode (various configurations)
- Coarse sync (full and narrow bandwidth)
- FFT operations (various sizes)
- Spectra computation

## Initial Performance Baseline

### Full Decode (15-second recording, 22 messages)

| Configuration | Time | Notes |
|---------------|------|-------|
| Default config | **~4.6 seconds** | Decodes all candidates |
| Top 10 candidates | **~1.8 seconds** | Limited candidate search |
| Narrow freq range | **~4.8 seconds** | 1000-2000 Hz only |

**Key Insight**: Default decode takes ~4.6 seconds for a 15-second recording. This is significantly slower than WSJT-X's typical 1-2 second decode time.

### Component Performance

| Component | Time | Notes |
|-----------|------|-------|
| Coarse sync (full BW) | 5.9-6.2 ms | Single call |
| Coarse sync (narrow BW) | 4.3-4.4 ms | 1000-2000 Hz |
| FFT-512 | 1.2 µs | Per transform |
| FFT-1024 | 2.5 µs | Per transform |
| FFT-2048 | 7.8 µs | Per transform |

### Analysis

**Bottleneck Identification** (Preliminary):
1. **Full decode pipeline** - ~4.6s is dominated by multiple passes through candidates
2. **Coarse sync** - 5-6ms per call, likely called multiple times
3. **Unknown stages** - Need to profile to identify LDPC and symbol extraction time

**Performance Gap**:
- Current: ~4.6 seconds
- WSJT-X target: ~1-2 seconds  
- **Gap: 2.3-4.6x slower** than target

## Bottleneck Analysis ✅

### Top 3 Performance Bottlenecks Identified:

**1. Decode Loop Combinatorics** (CRITICAL)
- Processes 200 candidates by default
- Each candidate tries: 3 timing offsets × 4 LLR methods × 3 scales = **36 decode attempts**
- Only 20/200 candidates succeed → 180 candidates waste time
- **Impact**: 7,200 potential LDPC decode attempts (though early exits reduce this)

**2. Memory Allocations in Hot Path** (HIGH)
- `extract_symbols_all_llr`: Allocates 3200-element complex buffer per candidate
- `extract_symbols_all_llr`: Allocates 8×79 arrays for s8
- Each LDPC decode: Multiple bitvec allocations in OSD
- **Impact**: Thousands of allocations per decode

**3. LDPC OSD Complexity** (MEDIUM)
- OSD-6 (Deep mode) used for top 100 candidates
- Gaussian elimination + pattern generation for each candidate
- **Impact**: Computational overhead on weak signal candidates

### Code Hotspots Located:

| Location | Issue | Frequency |
|----------|-------|-----------|
| `decoder.rs:291` | `extract_symbols_all_llr` allocates 4 LLR vectors | 200-600× per pass |
| `decoder.rs:326` | `scaled_llr.to_vec()` clones LLR | Up to 7,200× per pass |
| `extract.rs:122` | `vec![(0.0, 0.0); 3200]` complex buffer | 200-600× per pass |
| `extract.rs:227-228` | `cs` and `s8` allocations | 200-600× per pass |
| `decode_osd.rs:543+` | Multiple bitvec allocations in OSD | Variable |

## Phase 2 Quick Wins Identified

### Priority 1: Reduce Decode Attempts
- ✅ Early exit on success (already implemented)
- [ ] Reduce timing offsets from 3 to 2 (0.0, +0.025 only)
- [ ] Try fastest LLR method first (llrd), skip others if it works
- [ ] Reduce scale factors from 3 to 2 (1.0, 1.5 only)
- **Expected gain**: 30-40% fewer decode attempts

### Priority 2: Buffer Reuse
- [ ] Pre-allocate LLR buffers, reuse across candidates
- [ ] Pool complex buffers for downsampling
- [ ] Reuse bitvec allocations in LDPC
- **Expected gain**: 10-20% from reduced allocation overhead

### Priority 3: LDPC Optimization
- [ ] Use BP-only for candidates ranked 100-200
- [ ] Cache Gaussian elimination results
- [ ] Early termination in OSD when patterns found
- **Expected gain**: 10-15% on OSD-heavy workloads

## Benchmark Commands

```bash
# Run all benchmarks
cargo bench --bench decode_pipeline

# Run quick benchmarks (faster)
cargo bench --bench decode_pipeline -- --quick

# Profile with flamegraph
cargo install flamegraph
cargo flamegraph --bench decode_pipeline -- --bench

# Memory profiling
cargo bench --bench decode_pipeline --profile-time 10
```

## Files Modified

- `benches/decode_pipeline.rs` - New criterion benchmarks
- `benches/parallel_benchmark.rs` - Updated imports
- `Cargo.toml` - Added criterion dependency
- `PERFORMANCE.md` - Performance optimization roadmap

## Status

✅ Phase 1 Step 2: Baseline measurements established  
🔄 Phase 1 Step 4: Profiling in progress  
⏸️  Phase 1 Step 5: Awaiting profiling results for bottleneck identification
