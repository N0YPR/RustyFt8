# Multi-Pass Decoding Strategy Analysis

## Performance Gap

- **RustyFt8**: 5-6 messages decoded
- **WSJT-X**: 22 messages decoded
- **Gap**: ~16 messages (73% missing)

## WSJT-X Multi-Pass Strategy

### 1. Per-Candidate Decoding (ft8b.f90)

**Passes per candidate: 4-8**

```
Pass 1: nsym=1 (single symbol, llra)
Pass 2: nsym=2 (dual symbol, llrb)
Pass 3: nsym=3 (triple symbol, llrc)
Pass 4: nsym=1 normalized (llrd)
Pass 5-8: A Priori decoding (if QSO context available)
```

**Key characteristics:**
- **Single LLR scale**: 2.83 (after normalization)
- **LLR normalization**: Normalizes to zero mean, unit variance
- **OSD strategy by depth**:
  - `ndepth=1`: BP only (`maxosd=-1`)
  - `ndepth=2`: BP + OSD-0 (`maxosd=0`)
  - `ndepth=3`: BP + OSD-2 (`maxosd=2`, `norder=2`)
- **Early exit**: Returns on first successful decode

### 2. Top-Level Multi-Pass (ft8_decode.f90)

**Passes: 2-3 depending on depth**

```fortran
do ipass=1,npass
  ! Pass 1: Initial decode
  ! Pass 2: After subtracting decoded signals
  ! Pass 3: After subtracting pass 2 signals (depth=3 only)

  call sync8(dd, ...)      ! Find candidates

  do icand=1,ncand
    call ft8b(...)         ! Decode candidate

    if(success) then
      ! Subtract signal from audio
      call subtractft8(dd, itone, f1, xdt)
    endif
  enddo
enddo
```

**Key characteristics:**
- **Signal subtraction**: After each decode, removes signal from audio
- **Reveals masked signals**: Weaker signals become visible
- **Variable sync thresholds**: 1.3 → 1.6 → 2.0 across passes

### 3. A Priori Decoding (passes 5-8)

Uses QSO context to guide decoder:
- **iaptype=1**: CQ ??? ??? (32 AP bits)
- **iaptype=2**: MyCall ??? ??? (32 AP bits)
- **iaptype=3**: MyCall DxCall ??? (61 AP bits)
- **iaptype=4**: MyCall DxCall RRR (77 AP bits)
- **iaptype=5**: MyCall DxCall 73 (77 AP bits)
- **iaptype=6**: MyCall DxCall RR73 (77 AP bits)

## RustyFt8 Current Strategy

### Per-Candidate Decoding (decoder.rs)

**Attempts per candidate: ~60**

```rust
for nsym in [1, 2, 3] {
  extract_symbols(..., nsym, &mut llr);

  // BP: 3 × 16 = 48 attempts
  for scale in [1.0, 1.5, 0.75, 2.0, 0.5, ...] { // 16 values
    scaled_llr = llr * scale;
    if let Some(decoded) = ldpc::decode(&scaled_llr, 200) {
      return decoded; // Early exit
    }
  }

  // OSD: 3 × 4 = 12 attempts (nsym=1 only)
  if nsym == 1 {
    for osd_order in [0, 1, 2] {
      for osd_scale in [1.0, 1.5, 0.75, 2.0] {
        if let Some(decoded) = ldpc::osd_decode(&scaled_llr, osd_order) {
          return decoded;
        }
      }
    }
  }
}
```

**Missing:**
- ❌ LLR normalization
- ❌ Signal subtraction
- ❌ Top-level multi-pass
- ❌ A Priori decoding
- ⚠️ Trying too many LLR scales (16) vs WSJT-X's single normalized scale

## Critical Differences

### 1. LLR Normalization (WSJT-X wins)

**WSJT-X:**
```fortran
call normalizebmet(bmeta, 174)  ! Zero mean, unit variance
scalefac = 2.83
llra = scalefac * bmeta
```

**RustyFt8:**
```rust
// No normalization!
for scale in [1.0, 1.5, 0.75, ...] {  // Brute force search
  scaled_llr = llr * scale;
}
```

**Impact:** Poor LLRs lead to BP failures and OSD ineffectiveness

### 2. Signal Subtraction (CRITICAL - WSJT-X wins)

**WSJT-X:**
- After each successful decode, removes signal from audio
- Reveals weaker signals masked by stronger ones
- **Estimated impact: +8-12 decodes**

**RustyFt8:**
- No signal subtraction
- Weaker signals remain masked by stronger ones

**Impact:** Missing ~50% of decodable signals

### 3. Multi-Pass Strategy (WSJT-X wins)

**WSJT-X:**
- 2-3 passes at top level
- Different sync thresholds per pass
- Signal subtraction between passes

**RustyFt8:**
- Single pass through candidates
- No subtraction
- No threshold variation

### 4. A Priori Decoding (WSJT-X wins)

**WSJT-X:**
- Uses QSO context (mycall, hiscall, grid)
- Guides decoder for marginal signals
- **Estimated impact: +2-4 decodes**

**RustyFt8:**
- No A Priori decoding

## Why OSD Doesn't Help (Yet)

1. **Poor LLR quality without normalization**
   - Median |LLR| = 0.301 (should be ~1.0 after normalization)
   - BP fails → hard decisions are ~39 bits wrong
   - OSD can't fix 39-bit errors (only handles 1-3 bit errors)

2. **Missing the easy signals**
   - Signal subtraction would reveal 8-12 easier signals
   - OSD would help with those marginal cases
   - Currently, all BP failures are way too noisy for OSD

3. **Wrong signals being attempted**
   - Weaker signals are masked by stronger ones
   - OSD tries to decode noise instead of masked signals

## Recommended Implementation Order

### Phase 1: LLR Normalization (Expected: +0-1 decodes)
- Implement `normalize_llr()` function
- Use single scale factor (2.83)
- Remove LLR scale search

### Phase 2: Signal Subtraction (Expected: +8-12 decodes → 14-18 total)
- Implement `subtract_signal()` function
- Multi-pass with subtraction between passes
- This is THE critical missing piece

### Phase 3: OSD Integration (Expected: +2-4 decodes → 16-22 total)
- OSD becomes useful after signal subtraction
- Helps with marginal signals revealed by subtraction

### Phase 4: A Priori Decoding (Expected: +1-2 decodes → 17-23 total)
- Optional QSO context guidance
- Requires user input or QSO tracking

## Performance Targets

| Implementation | Expected Decodes | % of WSJT-X |
|----------------|------------------|-------------|
| Current (BP only) | 6 | 27% |
| + LLR normalization | 7 | 32% |
| + Signal subtraction | 14-18 | 64-82% |
| + OSD | 16-22 | 73-100% |
| + A Priori | 17-23 | 77-105% |

## Conclusion

**Signal subtraction is the critical missing piece, not OSD.**

OSD is correctly implemented but ineffective because:
1. LLRs aren't normalized (quality issue)
2. Signal subtraction hasn't revealed the easier masked signals
3. All current BP failures are too noisy for OSD to fix

Next steps:
1. ✅ **Implement LLR normalization**
2. ✅ **Implement signal subtraction** (biggest impact)
3. Re-test OSD effectiveness after subtraction
4. Consider A Priori decoding for final gap
