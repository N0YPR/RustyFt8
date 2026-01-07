# Performance Optimization Roadmap

## Current Status

RustyFT8 has a complete working decoder but needs performance optimization to approach WSJT-X speed.

**Test Coverage**: 191 tests passing with comprehensive message type coverage

## Performance Goals

1. **Match WSJT-X decode speed** (~1-2 seconds per 15-second recording)
2. **Minimize memory allocations** in hot paths
3. **Optimize critical loops** in sync and LDPC stages

## Profiling Strategy

### Step 1: Benchmark Current Performance

Create baseline measurements:
- Decode time for test recordings (210703_133430.wav, 181201_180245.wav)
- Memory allocation profile
- CPU usage breakdown by module

**Action**: Create benchmark suite using criterion.rs

### Step 2: Profile Hot Paths

Use profiling tools to identify bottlenecks:
- `cargo flamegraph` for CPU profiling
- `cargo bench` with criterion for micro-benchmarks
- Compare with WSJT-X timing breakdowns

**Expected hot spots**:
- FFT operations (sync stage)
- LDPC decoding (BP and OSD)
- Symbol extraction and downsampling
- Coarse/fine sync searches

### Step 3: Optimize by Module

#### A. Synchronization Stage (`src/sync/`)

**Potential optimizations**:
- [ ] Pre-allocate FFT buffers and reuse across candidates
- [ ] SIMD for complex number operations in downsampling
- [ ] Reduce allocations in `sync8d` equivalent
- [ ] Cache FFT plans instead of recreating
- [ ] Parallelize coarse sync candidate evaluation

**Files to focus on**:
- `src/sync/coarse.rs` - Coarse sync search
- `src/sync/fine.rs` - Fine frequency refinement  
- `src/sync/extract.rs` - Symbol extraction and downsampling
- `src/sync/fft.rs` - FFT operations

#### B. LDPC Decoding (`src/ldpc/`)

**Potential optimizations**:
- [ ] Optimize belief propagation inner loops
- [ ] Reduce allocations in OSD (ordered statistics decoding)
- [ ] Use fixed-size arrays where possible
- [ ] SIMD for LLR computations
- [ ] Early termination when codeword found

**Files to focus on**:
- `src/ldpc/decode_bp.rs` - Belief propagation
- `src/ldpc/decode_osd.rs` - Ordered statistics decoding
- `src/ldpc/decode_hybrid.rs` - Hybrid decoder

#### C. Symbol Extraction (`src/sync/extract.rs`)

**Potential optimizations**:
- [ ] Optimize complex multiply operations
- [ ] Reduce Vec allocations in hot loops
- [ ] Pre-compute phase rotations
- [ ] SIMD for dot products in LLR calculation
- [ ] Cache FFT results where applicable

#### D. Message Decoding (`src/message/`)

**Lower priority** - typically fast already:
- Message unpacking is mostly bit manipulation (fast)
- Cache is already optimized with FIFO

## Benchmark Framework

### Targets to Measure

```rust
// Criterion benchmarks to create:
1. Full decode pipeline (end-to-end)
2. Coarse sync only
3. Fine sync only
4. Symbol extraction only
5. LDPC decode only (BP iterations)
6. LDPC decode only (OSD)
7. FFT operations (various sizes)
```

### Comparison Metrics

For each test recording:
- **Time**: Total decode time vs WSJT-X
- **Allocations**: Heap allocations per decode
- **Throughput**: Messages decoded per second
- **Memory**: Peak memory usage

## Implementation Plan

### Phase 1: Measurement (Week 1)

1. ✅ Clean up outdated docs
2. [ ] Set up criterion benchmarks
3. [ ] Baseline current performance
4. [ ] Profile with flamegraph
5. [ ] Identify top 3 bottlenecks

### Phase 2: Quick Wins (Week 1-2)

Focus on high-impact, low-effort optimizations:
- [ ] Remove unnecessary allocations in hot loops
- [ ] Pre-allocate buffers that are reused
- [ ] Use `Vec::with_capacity` where size is known
- [ ] Replace `clone()` with borrows where safe

### Phase 3: Algorithmic Optimization (Week 2-3)

Focus on critical algorithms:
- [ ] Optimize FFT usage (buffer reuse, plan caching)
- [ ] LDPC inner loop optimization
- [ ] Symbol extraction vectorization
- [ ] Parallelize independent candidates

### Phase 4: SIMD/Parallelism (Week 3-4)

Advanced optimizations:
- [ ] SIMD for complex number operations
- [ ] Parallel candidate evaluation with rayon
- [ ] Multi-threaded coarse sync
- [ ] Vectorized LLR calculations

### Phase 5: Final Tuning (Week 4)

- [ ] Fine-tune parameters (iteration counts, thresholds)
- [ ] Memory pool for common allocations
- [ ] Profile-guided optimization (PGO)
- [ ] Compare final results with WSJT-X

## Success Criteria

**Minimum**:
- Decode time within 2x of WSJT-X
- No accuracy regression (maintain current decode success rate)
- All tests continue to pass

**Target**:
- Decode time within 1.5x of WSJT-X
- Memory usage < WSJT-X
- Decode accuracy matches or exceeds WSJT-X

**Stretch**:
- Decode time matches WSJT-X
- Real-time decoding capability
- Lower memory footprint than WSJT-X

## Resources

- WSJT-X source: `./wsjtx/wsjtx-2.7.0/`
- Test recordings: `tests/test_data/`
- Existing benchmarks: `benches/parallel_benchmark.rs`

## Notes

- Keep reference docs: AP_DECODING_EXPLAINED.md, WSJT-X_DECODER_STRATEGY.md, MULTIPASS_ANALYSIS.md, MULTIPASS_STATUS.md, HYBRID_DECODER_RESULTS.md
- Focus on profiling-driven optimization (measure, don't guess)
- Maintain code clarity - only optimize proven bottlenecks
- Document performance-critical sections
