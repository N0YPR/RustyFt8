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

## Next Steps

### 1. Complete Profiling ✓ (in progress)
- [x] Set up criterion benchmarks
- [x] Establish baseline measurements
- [ ] Profile with flamegraph to identify hot functions
- [ ] Measure allocations with memory profiler

### 2. Identify Top 3 Bottlenecks
Based on preliminary results, likely candidates:
1. LDPC decoding (BP + OSD iterations)
2. Symbol extraction and downsampling
3. Multiple decode passes (multipass strategy)

### 3. Quick Wins to Target
- Reduce unnecessary allocations in hot loops
- Pre-allocate buffers for reuse
- Cache FFT plans instead of recreating
- Optimize candidate filtering

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
