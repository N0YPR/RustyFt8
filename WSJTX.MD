# WSJT-X Authoritative Sources

This document serves as a reference for the official WSJT-X implementation when developing and validating the FT8/FT4 implementation in this project. It is inteded for use by GitHub CoPilot or other AI assistants to assist in development of this project.

## Notes for AI Assistants

When asked about FT8, FT4, or WSJT-X implementation details:

1. **Reference this document first** for the correct authoritative source
2. **Use the SourceForge URLs** provided above  
3. **Acknowledge when SourceForge access isn't available** rather than guessing
4. **Suggest consulting the specific Fortran files** mentioned for detailed algorithms
5. **Recommend test vector validation** against the reference implementation

This ensures accurate, authoritative answers based on the actual WSJT-X implementation rather than assumptions or incomplete information.

## Primary Authoritative Source

**QEX article FT4 & FT8 protocol**: https://wsjt.sourceforge.io/FT4_FT8_QEX.pdf

**Official WSJT-X Repository**: https://sourceforge.net/p/wsjt/wsjtx/ci/master/tree/

This is the definitive source maintained by K1JT (Joe Taylor) and the WSJT development team. All FT8/FT4 LDPC implementations, test vectors, and behavioral questions should reference this source.

## Accessing the Source

### Direct Browser Access
Navigate to: https://sourceforge.net/p/wsjt/wsjtx/ci/master/tree/

### Specific File URLs
Base URL: `https://sourceforge.net/p/wsjt/wsjtx/ci/master/tree/lib/`
Source Code: `https://sourceforge.net/projects/wsjt/files/wsjtx-2.7.0/wsjtx-2.7.0.tgz/download`

### Local files

If you need to download source in order to compile it, download it into the ./wsjtx folder that has been added to the .gitignore file.

## Building WSJT-X from Source

### Prerequisites

The dev container already has the necessary dependencies installed. If building elsewhere, ensure these are available:

```bash
# Required packages (already in devcontainer.json)
cmake
build-essential
gfortran
libboost-all-dev
libfftw3-dev
libudev-dev
libusb-1.0-0-dev
qtbase5-dev
qtmultimedia5-dev
qttools5-dev
libqt5multimedia5-plugins
libqt5serialport5-dev
```

### Build Steps

1. **Download source** (if not already present):
   ```bash
   cd /workspaces/RustyFt8/wsjtx
   wget https://sourceforge.net/projects/wsjt/files/wsjtx-2.7.0/wsjtx-2.7.0.tgz/download -O wsjtx-2.7.0.tgz
   tar -xzf wsjtx-2.7.0.tgz
   cd wsjtx-2.7.0
   ```

2. **Configure with CMake** (skip docs and manpages for build environments):
   ```bash
   mkdir -p build && cd build
   cmake .. -DCMAKE_BUILD_TYPE=Release -DWSJT_GENERATE_DOCS=OFF -DWSJT_SKIP_MANPAGES=ON
   ```

3. **Build** (uses all available CPU cores):
   ```bash
   make -j$(nproc)
   ```

4. **Verify build artifacts**:
   ```bash
   ls -lh wsjtx-prefix/src/wsjtx-build/ | grep -E "wsjtx$|ft8|jt9$"
   ```

### Key Build Artifacts

After successful build, you'll find these executables in `build/wsjtx-prefix/src/wsjtx-build/`:

- **`wsjtx`** (~18MB) - Main WSJT-X application with full FT8 support
- **`jt9`** (~1.5MB) - Decoder for FT8 and other modes
- **`ft8code`** (~229KB) - FT8 encoder utility
- **`ft8sim`** (~230KB) - FT8 simulator for testing signals
- **`ft4code`**, **`ft4sim`** - FT4 equivalents

### Common Build Issues

1. **Missing FFTW3**: Ensure `libfftw3-dev` is installed
2. **Documentation errors**: Always use `-DWSJT_GENERATE_DOCS=OFF -DWSJT_SKIP_MANPAGES=ON` flags
3. **Qt5 not found**: Verify all Qt5 development packages are installed

### Using Built Tools

The compiled tools can be used for:
- Testing encoding/decoding algorithms
- Generating test vectors
- Validating RustyFt8 implementation
- Understanding reference behavior

### FT8 Simulation and Test Tools
- **Main Simulator**: `lib/ft8/ft8sim.f90`  
  - Complete FT8 simulation environment
  - Message encoding to symbol generation
  - Channel simulation with noise and fading

#### Running ft8sim

**IMPORTANT**: Always run ft8sim from the `/workspaces/RustyFt8/wsjtx` directory to keep generated WAV files out of the git repository (wsjtx folder is in .gitignore).

**Usage**:
```bash
cd /workspaces/RustyFt8/wsjtx
../wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/ft8sim "MESSAGE" f0 DT fdop del nfiles snr
```

**Parameters**:
- `MESSAGE`: FT8 message to encode (e.g., "CQ SOTA N0YPR DM42")
- `f0`: Center frequency in Hz (typically 1500.0)
- `DT`: Time offset in seconds (typically 0.0)
- `fdop`: Doppler shift in Hz (typically 0.1)
- `del`: Delay parameter (typically 1.0)
- `nfiles`: Number of WAV files to generate (use 1 for single test)
- `snr`: Signal-to-noise ratio in dB (e.g., -10 for typical weak signal)

**Example**:
```bash
cd /workspaces/RustyFt8/wsjtx
../wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/ft8sim "CQ SOTA N0YPR DM42" 1500.0 0.0 0.1 1.0 1 -10
```

This generates `000000_000001.wav` in the wsjtx directory with the encoded FT8 signal.

#### Using ft8code for Message Encoding Details

**IMPORTANT**: The `ft8code` tool provides detailed information about FT8 message encoding, including source encoding, LDPC parity bits, and channel symbols (tones). This is invaluable for understanding and validating FT8 implementations.

**Usage**:
```bash
ft8code "MESSAGE"    # Get encoding details for a specific message
ft8code -T           # Show examples of all message types
ft8code -t           # Show short format examples
```

**Output includes**:
- **Source-encoded message**: 77-bit payload after message packing
- **14-bit CRC**: Cyclic redundancy check for error detection
- **83 Parity bits**: LDPC(174,91) forward error correction bits
- **Channel symbols**: 79 tones (8-FSK symbols 0-7) representing the complete transmission
  - 3 sync sequences of 7 symbols each (Costas arrays): positions 0-6, 36-42, 72-78
  - 58 data symbols carrying the encoded message

**Example**:
```bash
/workspaces/RustyFt8/wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/ft8code "CQ N0YPR DM42"
```

**Output format**:
```
Channel symbols (79 tones):
  Sync               Data               Sync               Data               Sync
3140652 00000000100611657160217552423 3140652 07612100650134045111173054306 3140652
```

The 79 symbols represent the complete FT8 transmission where:
- Each digit (0-7) represents one of 8 frequency tones
- The transmission takes 12.64 seconds (79 symbols × 0.16s per symbol)
- The sync patterns (3140652) are Costas arrays for time/frequency synchronization
