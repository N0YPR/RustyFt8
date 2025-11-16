# RustyFt8

A Rust implementation of FT8, the digital weak-signal communication mode, featuring:
- **Complete transmit chain**: Message encoding, LDPC FEC, GFSK modulation
- **Partial receive chain**: Near-perfect synchronization, symbol extraction (21/21 Costas validation)
- **no_std compatible**: Works in embedded environments with `alloc`

FT8 achieves robust communication at -21 dB SNR using 8-FSK modulation, LDPC(174,91) error correction, and Costas array synchronization patterns.

## 🔬 How FT8 Works

**Transmit Chain** (Message → WAV):
```
Text "CQ W1ABC FN42"
  → 77-bit pack → +14-bit CRC → 91 bits
  → LDPC encode → +83 parity → 174 bits
  → 3 bits/symbol → 58 data symbols (0-7)
  → +21 Costas sync → 79 symbols total
  → GFSK modulation → 12.64s audio @ 12kHz
```

**Receive Chain** (WAV → Message):
```
15s audio @ 12kHz
  → FFT spectra → 2D Costas correlation
  → Coarse sync → frequency/time candidates
  → Fine sync → ±2.5 Hz, ±20 ms refinement
  → Symbol extract → 79×8 tone magnitudes
  → Soft decode → 174 LLRs (⚠️ needs improvement)
  → LDPC decode → 91 bits → check CRC
  → Unpack 77 bits → Text message
```

**Key Parameters**:
- 8-FSK: 8 tones spaced 6.25 Hz apart
- Symbol rate: 6.25 baud (0.16s/symbol)
- Duration: 79 symbols × 0.16s = 12.64 seconds
- Bandwidth: ~50 Hz
- Sync: 3× Costas arrays (pattern `[3,1,4,0,6,5,2]`)

## 📚 Key Documentation

**For AI Assistants & Developers**, please read:

- **[`AGENTS.md`](AGENTS.md)** - Comprehensive project guide
  - Project overview and conventions
  - Development workflow and testing strategy
  - WSJT-X reference implementation (build instructions and tool usage)
  - FT8 protocol reference

Failure to follow these guidelines may result in incorrect implementations or test failures.

## 📊 Current Status

### What Works

✅ **Transmit Chain (100%)** - Complete message → WAV pipeline validated against WSJT-X
✅ **Coarse Sync** - 2D FFT-based Costas correlation matches WSJT-X candidate detection
✅ **Fine Sync** - Sub-Hz frequency (±2.5 Hz) and sub-ms timing (±20 ms) accuracy
✅ **Symbol Extraction** - Perfect 21/21 Costas validation proves correct timing
✅ **LDPC Decoder** - Belief propagation with 130 passing tests
✅ **End-to-End Decode** - **Working! Minimum SNR: -15 dB**

### Performance Results

**Tested SNR Range**: -24 dB to +10 dB
**Decoder**: Single-symbol soft decoding (nsym=1)

| SNR (dB) | Status | LDPC Iterations | Notes |
|----------|--------|-----------------|-------|
| +10 to Perfect | ✅ Pass | 1 | Instant decode |
| -10 | ✅ Pass | 16 | Strong decode |
| **-12** | ✅ Pass | 3 | Good sync (19/21 Costas) |
| **-15** | ✅ Pass | 21 | Marginal sync (19/21 Costas) |
| -18 and below | ❌ Fail | - | Sync quality insufficient |

**Minimum Working SNR**: **-15 dB** (vs WSJT-X: -21 dB)
**Performance Gap**: ~6 dB (expected with single-symbol decoding)

See [`docs/SNR_TESTING.md`](docs/SNR_TESTING.md) for detailed test results.

### What Needs Work

🚧 **Multi-Symbol Soft Decoding** - nsym=2/3 implemented but not working yet (under investigation)
⚠️  **Low SNR Performance** - Need nsym=2/3 to reach -18 to -21 dB like WSJT-X

## 🚀 Next Steps

### 1. Debug Multi-Symbol Soft Decoding (Critical Priority - In Progress)

**Status**: nsym=2 and nsym=3 implemented in [src/sync.rs](src/sync.rs) but not decoding

**Current Implementation**:
- ✅ nsym=1: Working, -15 dB minimum SNR
- 🚧 nsym=2: Implemented but LDPC fails even on perfect signals
- 🚧 nsym=3: Implemented but has issues (29 symbols don't divide evenly by 3)

**Root Cause Found**: Fine frequency synchronization has ~1.5 Hz systematic error
- Signal at 1500 Hz detected at 1501.5 Hz (+1.5 Hz error)
- With 6.25 Hz tone spacing, this causes tone detection errors
- nsym=1 tolerates ~10 bit errors (LDPC corrects) ✅
- nsym=2 produces ~20+ bit errors (exceeds LDPC) ❌
- Coherent combining amplifies frequency errors across symbol pairs

**Solution**: Improve fine sync to sub-Hz accuracy
- Current: ±2.5 Hz range with 0.25 Hz steps
- Needed: Better frequency estimation algorithm
- **Expected result**: -18 dB SNR with nsym=2, -21 dB with nsym=3

### 2. Testing & Benchmarks
- ✅ SNR sweep testing (-24 to +10 dB) completed
- ✅ Established minimum SNR threshold (-15 dB)
- ⏳ Add automated integration tests for encode→decode round trips
- ⏳ Test with different message types and conditions

### 3. Clean Up & Polish
- Make debug output conditional on `--verbose` flag ([src/sync.rs](src/sync.rs))
- Remove temporary workarounds in [ft8detect.rs:163-166](src/bin/ft8detect.rs#L163-L166), [sync.rs:805](src/sync.rs#L805)
- Remove debug code from hot paths (FFT, correlation loops)

### 4. Real-Time Operation
- Live audio input (ALSA/PulseAudio/PortAudio)
- Sliding window for continuous monitoring
- Process 15-second intervals in real-time

### 5. Feature Completeness
- All FT8 message types (compound callsigns, contest modes)
- Callsign hash cache integration for decoding
- Transmit path integration (already have pulse shaping and modulation)

### 6. Optimization & Production
- Profile and optimize hot paths (FFT, correlation)
- Consider SIMD optimizations
- Evaluate using rustfft/realfft crate
- Stabilize public API and add versioning

## 📈 Roadmap

**Now**: Debug nsym=2 → improve to -18 dB SNR
**Next**: Real-time operation → live audio monitoring
**Then**: Feature completeness → all message types, hash cache
**Future**: Production polish → optimization, docs, API stability

**Current Achievement**: -15 dB minimum SNR (sufficient for most real-world FT8 operation)
