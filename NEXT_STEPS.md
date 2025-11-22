# Next Steps: Real Recording Decode Investigation

**Last Updated**: 2025-11-22 (Post-Fix)
**Current Status**: 11/22 messages decoded (50%)

## 🎉 Major Fix Implemented

### Critical Bug Found and Fixed: Double Normalization in Downsample

**Problem**: The `downsample_200hz` function in `src/sync/downsample.rs` was applying normalization TWICE:
1. IFFT automatically divides by N (3200) → see `src/sync/fft.rs:113`
2. Then multiplying by `1/sqrt(NFFT_IN * NFFT_OUT)` = 1/24,787

Total scaling: `(1/3200) × (1/24,787)` = **1/79,318,400** (essentially zero!)

**Fix**: Account for IFFT's built-in normalization:
```rust
// OLD (WRONG):
let fac = 1.0 / ((NFFT_IN * NFFT_OUT) as f32).sqrt();  // 0.00004

// NEW (CORRECT):
let fac = (NFFT_OUT as f32 / NFFT_IN as f32).sqrt();  // 0.129
```

**Result**:
- ✅ Fine sync now works (sync values > 0)
- ✅ Downsampled buffers have proper signal power
- ❌ Still only 11/22 messages (50%) vs WSJT-X's 22/22 (100%)

---

## Current Situation

### What Works ✅
- Downsampling with correct normalization
- Fine sync finding correlation peaks
- Weak signal decoding for many signals

### What Doesn't Work ❌
- **Still missing 11 strong signals** including:
  - `CQ F5RXL IN94` (should be easy)
  - `N1PJT HB9CQK -10` (strong signal)
  - `K1BZM EA3GP -09` (strong signal at 2695 Hz)
  - `KD2UGC F6GCP R-23`
  - And 7 more...

---

## Next Steps (Priority Order)

### Step 1: Re-enable Pipeline Logging for Failing Signals 🔴 HIGH PRIORITY

Now that fine sync is working, re-enable the detailed logging to track where specific strong signals fail:

```rust
// In src/sync/fine.rs
eprintln!("FINE_SYNC: freq={:.1} Hz, dt_in={:.2}s, sync_in={:.3}",
          candidate.frequency, candidate.time_offset, candidate.sync_power);
eprintln!("  REFINED: freq={:.1} Hz, dt_out={:.2}s, sync_out={:.3}",
          best_freq, refined_time, best_sync);

// In src/sync/extract.rs
eprintln!("EXTRACT: freq={:.1} Hz, dt={:.2}s, nsym={}", ...);
eprintln!("  Extracted: nsync={}/21, mean_abs_LLR={:.2}, max_LLR={:.2}", ...);
```

Focus on these **specific missing signals**:
- `K1BZM EA3GP -09` at ~2695 Hz
- `CQ F5RXL IN94` at ~1197 Hz
- `N1PJT HB9CQK -10` at ~466 Hz

**Look for**:
- Are candidates even generated for these frequencies?
- If yes, what are their sync scores and ranks?
- Do they pass fine sync?
- Are LLRs reasonable quality (mean_abs > 2.0)?
- Does LDPC try to decode them?

---

### Step 2: Compare Decoded vs Expected Messages 🟡 MEDIUM PRIORITY

**Analysis**:
```
WSJT-X: 22 messages
RustyFt8: 11 messages (50%)

Missing: 11 messages
Decoded correctly: 11 messages
```

**Compare characteristics**:
- Are we missing specific frequency ranges?
- Are we missing specific time offsets?
- Are we missing specific message types?
- What's the SNR distribution of missing vs decoded?

**Command**:
```bash
# Get WSJT-X decodes
wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/jt9 -8 -d 3 \
  tests/test_data/210703_133430.wav > wsjt_output.txt

# Compare with RustyFt8
cargo test --release --test real_ft8_recording test_real_ft8_recording_210703_133430 \
  -- --ignored --nocapture > rusty_output.txt
```

---

### Step 3: Investigate Candidate Selection 🟡 MEDIUM PRIORITY

**Hypothesis**: Good candidates might exist but aren't being selected for decoding.

**Check**:
1. Are candidates found for missing signals' frequencies?
2. What are their ranks in the candidate list?
3. Are they being filtered out by time offset penalty?
4. Should we increase `decode_top_n` from current value?

**Test**:
```bash
# Use debug tools to see all candidates
cargo run --release --example debug_candidates 2>&1 | grep "269[0-9]\|119[0-9]\|46[0-9]"
```

---

### Step 4: Review Time Offset Handling 🟢 LOW PRIORITY

The time offset penalty (in `src/sync/candidate.rs:55-86`) might be too aggressive for real recordings.

**Current approach**: Weighted penalty based on time offset
```rust
// Soft penalty: gradually reduce sync_power for large time offsets
let time_penalty = 1.0 - (time_offset.abs() / 2.5).min(1.0);
let adjusted_sync = sync_val * time_penalty;
```

**Alternative**: Try a hard cutoff instead (see Step 3 in previous notes)

---

### Step 5: Compare WSJT-X Algorithm Details 🟢 LOW PRIORITY

**Files to study**:
```
wsjtx/wsjtx-2.7.0/src/wsjtx/lib/ft8/
├── sync8.f90          # Coarse sync
├── ft8b.f90           # Main decode loop
├── sync8d.f90         # Fine sync
└── ft8_downsample.f90 # Downsampling (NOW FIXED!)
```

**Focus areas**:
1. Candidate selection threshold in `sync8.f90`
2. Peak selection algorithm
3. Multi-pass decoding strategy in `ft8b.f90`

---

## Technical Details

### Normalization Fix Details

The issue was in `/workspaces/RustyFt8/src/sync/downsample.rs:146-155`.

**Why the bug occurred**:
- WSJT-X comment said: `fac = 1.0/sqrt(float(NFFT1)*NFFT2)`
- But WSJT-X's IFFT probably doesn't normalize by 1/N
- Our `fft_complex_inverse` (using RustFFT) DOES normalize by 1/N
- So we need to account for that difference

**Correct formula derivation**:
```
Target: fac = 1/sqrt(NFFT_IN * NFFT_OUT)
But IFFT already divided by NFFT_OUT
So we need: fac' = NFFT_OUT/sqrt(NFFT_IN * NFFT_OUT)
           = sqrt(NFFT_OUT/NFFT_IN)
           = sqrt(3200/192000)
           ≈ 0.129
```

---

## Performance Tracking

| Milestone | Messages | Success Rate | Notes |
|-----------|----------|--------------|-------|
| Baseline | 8/19 | 42% | Before investigation |
| Time penalty | 10/19 | 53% | After time offset fixes |
| **Norm fix** | **11/22** | **50%** | **After downsample fix** |
| Target | 22/22 | 100% | Match WSJT-X |

Note: Different test expectations (19 vs 22) - need to verify reference list.

---

## Testing Commands

```bash
# Run failing test
cargo test --release --test real_ft8_recording test_real_ft8_recording_210703_133430 -- --ignored --nocapture

# Compare with WSJT-X
wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/jt9 -8 -d 3 tests/test_data/210703_133430.wav

# Debug candidates (with logging enabled)
cargo run --release --example debug_candidates

# Check git diff
git diff src/sync/downsample.rs
```

---

## Key Files Modified

- ✅ `src/sync/downsample.rs:151` - Fixed normalization factor
- 📝 `src/sync/fine.rs` - Added diagnostic logging (commented out)
- 📝 `src/sync/extract.rs` - Added diagnostic logging (commented out)

---

## Next Session TODO

1. **Uncomment** logging in fine.rs and extract.rs
2. **Run test** and capture output for the 11 missing signals
3. **Analyze** where each signal fails:
   - Not in candidate list?
   - Bad fine sync?
   - Poor LLRs?
   - LDPC doesn't converge?
4. **Compare** frequency/time distribution of decoded vs missing
5. **Investigate** why we have different expected counts (19 vs 22)

---

Good luck! 🚀
