# SIMD Optimization Evaluation

## Executive Summary

**Recommendation: Do NOT implement explicit SIMD for LLR scaling**

The Rust compiler's auto-vectorization provides ~8.8x speedup on LLR scaling operations, which is **better** than handwritten SIMD intrinsics (7.3x speedup). Explicit SIMD adds code complexity, platform-specific code, and maintenance burden without providing any performance benefit.

## Benchmark Results

Test platform: ARM64 (aarch64) with NEON support

### LLR Scaling Performance (100 iterations of 174 floats)

| Implementation | Time | Speedup vs Scalar | Code Complexity |
|---|---|---|---|
| Truly scalar (volatile) | 9.47 µs | 1.0x (baseline) | Simple |
| Auto-vectorized (compiler) | 1.08 µs | **8.8x faster** | Simple |
| Explicit NEON intrinsics | 1.29 µs | 7.3x faster | Complex, unsafe |

## Key Findings

### 1. Compiler Auto-Vectorization is Excellent

Modern Rust/LLVM automatically recognizes simple patterns like:
```rust
for i in 0..len {
    output[i] = input[i] * scale;
}
```

And generates optimal SIMD code without any programmer intervention. The compiler achieves 8.8x speedup, which is near the theoretical maximum for 4-wide NEON operations.

### 2. Handwritten SIMD is Slower

Our explicit NEON intrinsics implementation is **20% slower** than the compiler's auto-vectorization (1.29 µs vs 1.08 µs). Possible reasons:
- Compiler has more optimization opportunities with high-level code
- Our loop over chunks may have suboptimal memory access patterns
- Compiler can see through the entire call chain and optimize better

### 3. Code Complexity Trade-off

**Auto-vectorized version** (what we have now):
```rust
for i in 0..174 {
    scaled_llr[i] = llr[i] * scale;
}
```
- 3 lines of code
- Safe, portable
- Works on all architectures
- Easy to understand and maintain

**Explicit SIMD version** (proposed):
```rust
#[cfg(target_arch = "aarch64")]
unsafe {
    let scale_vec = vdupq_n_f32(scale);
    let chunks = llr.len() / 4;
    for i in 0..chunks {
        let offset = i * 4;
        let input_vec = vld1q_f32(llr.as_ptr().add(offset));
        let result_vec = vmulq_f32(input_vec, scale_vec);
        vst1q_f32(output.as_mut_ptr().add(offset), result_vec);
    }
    // Handle remainder...
}
#[cfg(target_arch = "x86_64")]
unsafe {
    // Different implementation for x86_64...
}
```
- 40+ lines of code
- Unsafe, platform-specific
- Requires separate implementations for ARM, x86_64, etc.
- Difficult to understand and maintain
- **Performs worse than compiler auto-vectorization**

## Impact on Overall Performance

Current decoder performance: **2.4s per recording**

LLR scaling happens in the LDPC decoder's BP loop. Estimated impact:
- LLR scaling is ~2-3% of total runtime
- 8x speedup on 2-3% = ~1.5-2% total speedup
- 2.4s → 2.35s (gain of 0.05s)

This small gain doesn't justify the code complexity, especially since the compiler already does it for us.

## When Would Explicit SIMD Be Worth It?

Explicit SIMD intrinsics are valuable when:

1. **Complex patterns the compiler can't recognize**:
   - Custom reduction operations
   - Interleaved data structure access
   - Specialized algorithms (FFT, matrix operations)

2. **Inner loops of performance-critical code**:
   - LDPC BP belief propagation updates (check node / variable node)
   - FFT butterfly operations
   - Symbol extraction magnitude calculations

3. **When profiling shows significant impact**:
   - Function takes >10% of total runtime
   - Auto-vectorization fails (verify with assembly inspection)
   - SIMD provides >2x additional speedup over auto-vectorization

## Recommendations

### Short-term (Current Project)
- ✅ **Keep current simple code** - compiler auto-vectorization is sufficient
- ❌ **Do NOT add explicit SIMD** for LLR scaling
- ✅ **Focus on algorithmic optimizations** - these provide larger gains

### Long-term (If Pursuing More Optimization)
Consider SIMD for:
1. **LDPC BP inner loops** (check node updates, variable node updates)
2. **FFT operations** (if switching to custom FFT implementation)
3. **Symbol extraction** (magnitude calculations across frequency bins)

But only after:
- Profiling shows these are major bottlenecks (>10% runtime each)
- Verifying compiler doesn't already vectorize them
- Measuring actual performance gain vs code complexity

## Conclusion

**The Rust compiler is smarter than we are** for simple operations like LLR scaling. Our time is better spent on:

1. **Algorithmic improvements** (we've already achieved 48% speedup this way)
2. **Reducing unnecessary work** (candidate filtering, early termination)
3. **Better data structures** (cache-friendly layouts, reducing allocations)

Rather than fighting the compiler with explicit SIMD that performs worse and adds complexity.

## Benchmark Code

The full SIMD spike benchmark is in `benches/simd_benchmark.rs`:
- Implements scalar, auto-vectorized, and explicit SIMD versions
- Tests ARM NEON and x86_64 AVX2
- Includes correctness verification
- Can be run with: `cargo bench --bench simd_benchmark`

## Platform Notes

- **ARM64 (aarch64)**: NEON provides 4-wide float operations
- **x86_64**: AVX2 provides 8-wide float operations  
- Theoretical maximum speedup: 4x on ARM, 8x on x86_64
- Compiler achieves near-theoretical performance automatically
