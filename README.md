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

### What Needs Work

⚠️  **Soft Demodulation** - Single-symbol approach limits SNR performance (see below)
⚠️  **End-to-End Decode** - LDPC doesn't converge on low-SNR signals due to weak LLRs

### The Problem: Single-Symbol vs Multi-Symbol Soft Decoding

**Current approach** (single-symbol):
```rust
LLR = magnitude(symbol_k_tone_1) - magnitude(symbol_k_tone_0)
```

**WSJT-X approach** (multi-symbol):
```rust
LLR = magnitude(symbol_k + symbol_k+1 + symbol_k+2) - magnitude(...)
// Coherently combines 2-3 symbols before taking magnitude
// Provides ~3-6 dB SNR improvement
```

**Impact**:
- Perfect 21/21 Costas sync proves signal processing and timing are correct
- LDPC decoder has weak LLR inputs, preventing convergence at low SNR
- Minimum SNR unknown (needs testing); WSJT-X achieves -21 dB

**Next step**: Implement multi-symbol soft decoding (see below)

## 🚀 Next Steps

### 1. Multi-Symbol Soft Decoding (Critical Priority)

**Implementation** (from WSJT-X `ft8b.f90`):
```fortran
! Sum complex values of 2-3 consecutive symbols, then take magnitude
s2(i) = abs(cs(graymap(i1),ks) + cs(graymap(i2),ks+1) + cs(graymap(i3),ks+2))
```

- Test all 8³ = 512 possible 3-symbol combinations
- Sum complex symbol values coherently before computing magnitude
- Choose maximum magnitude combination as most likely sequence
- **Files to modify**: [src/sync.rs:1044-1090](src/sync.rs#L1044-L1090)
- **Expected result**: -15 to -20 dB SNR decode capability (vs current: unknown, likely >0 dB)

### 2. Testing & Benchmarks
- Generate test signals at varying SNR using WSJT-X's `ft8sim`
- Establish minimum SNR threshold and decode success rates (-24 to 0 dB)
- Add automated integration tests for encode→decode round trips
- Compare performance to WSJT-X baseline

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

**Now**: Multi-symbol soft decoding → unlock low-SNR decode
**Next**: Testing & benchmarks → validate performance
**Then**: Real-time operation → live audio monitoring
**Future**: Production polish → optimization, docs, API stability
