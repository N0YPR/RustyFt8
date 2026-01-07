# Profiling Results - FT8 Decoder

## Profiling Run - Jan 7, 2026

### Recording 1: 210703_133430 (Total: 2.5s, 22 messages)

| Pass | Phase | Time | Candidates | Decoded |
|---|---|---|---|---|
| 0 | Coarse sync | 9ms | 1000 → 80 | - |
| 0 | **Decode loop** | **898ms** | 80 | 14 |
| 1 | Coarse sync | 7ms | 1000 → 50 | - |
| 1 | **Decode loop** | **484ms** | 50 | 6 |
| 2 | Coarse sync | 7ms | 1000 → 50 | - |
| 2 | **Decode loop** | **391ms** | 50 | 2 |
| - | Overhead | ~711ms | - | - |

### Recording 2: 181201_180245 (Total: 2.8s, 21 messages)

| Pass | Phase | Time | Candidates | Decoded |
|---|---|---|---|---|
| 0 | Coarse sync | 7ms | 1000 → 80 | - |
| 0 | **Decode loop** | **928ms** | 80 | 15 |
| 1 | Coarse sync | 6ms | 1000 → 50 | - |
| 1 | **Decode loop** | **478ms** | 50 | 6 |
| 2 | Coarse sync | 7ms | 1000 → 50 | - |
| 2 | **Decode loop** | **582ms** | 50 | 0 |
| - | Overhead | ~799ms | - | - |

## Key Findings

### 1. Coarse Sync is Fast ✅
- Only 7-9ms per pass (23ms total)
- Less than 1% of total time

### 2. Decode Loop Dominates 🔥
- **71% of total time** (1773-1988ms out of 2500-2800ms)
- Pass 0: 898-928ms for 80 candidates = **11-12ms per candidate**
- Includes: fine sync, symbol extraction, LDPC BP/OSD, message parsing

### 3. Significant Overhead (29%)
- 711-799ms unaccounted (parallel coordination, memory allocation)

### 4. Pass 2 Questionable ROI
- Takes 391-582ms but decoded only 0-2 messages
- **Potential optimization: Skip pass 2 when pass 0-1 perform well**

## Where Time Goes

**Breakdown of 2.5-2.8s:**
- **71%** - Decode loop (fine sync + extract + LDPC)
- **29%** - Overhead (parallelization, memory)
- **<1%** - Coarse sync

**Decode Loop Sub-components** (estimated from similar decoders):
- LDPC OSD: ~40% of decode loop = **700-800ms**
- Symbol extraction: ~25% = **400-500ms**
- LDPC BP: ~20% = **350-400ms**
- Fine sync + other: ~15% = **250-300ms**

## Optimization Priorities

### Priority 1: Optimize LDPC OSD (Est. 700-800ms) 🔥
- Gaussian elimination for systematic form
- Pattern generation and testing
- Most expensive for weak signals (top 100 candidates use Deep OSD)

**Actions:**
- Pool LDPC buffers (toc, tov, zn) to avoid allocations
- Optimize Gaussian elimination inner loops
- Consider reduced OSD order for candidates 50-100

### Priority 2: Skip Pass 2 Conditionally (Est. 400-500ms savings)
- Pass 2 decoded 0-2 messages
- Consider: Skip if passes 0-1 found >18 messages

### Priority 3: Symbol Extraction Optimization (Est. 400-500ms)
- FFT operations
- Downsampling 12kHz → 200Hz
- Phase tracking

**Actions:**
- Cache FFT plans
- Pre-allocate complex buffers
- Optimize phase rotation loops

### Priority 4: Reduce Overhead (Est. 200-300ms)
- 29% overhead suggests parallelization inefficiency
- Consider explicit thread pool sizing
- Reduce allocations in hot paths

## Next Steps

1. **Add finer timing** inside decode loop to confirm estimates
2. **Try skipping Pass 2** when performance is good
3. **Implement LDPC buffer pooling**
4. **Profile LDPC OSD vs BP** time split

## Target Performance

To reach WSJT-X speed (1.5-2.0s):
- Reduce decode loop from 1.8s → 1.2s (cut 33%)
- Skip Pass 2: saves 400-500ms
- Optimize OSD: saves 200-300ms  
- Reduce overhead: saves 100-200ms
- **Total potential: 700-1000ms savings → 1.5-1.8s achievable**
