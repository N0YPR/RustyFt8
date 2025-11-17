# OSD Quick Reference

## The Core Problem

**Current:** Bit-flip approach doesn't respect LDPC code structure
**Solution:** Use generator matrix transformation with Gaussian elimination

## Key Insight

After ordering bits by reliability and performing Gaussian elimination:
```
Most reliable K bits → Information bits (input to encoder)
Remaining N-K bits → Derived via parity constraints
```

## Critical Algorithm Steps

### 1. Generate Generator Matrix (91×174)
Each row = encoding of a unit information vector

### 2. Order Bits by |LLR|
Most reliable first → Use as information bits

### 3. Gaussian Elimination in GF(2)  ⭐ **THIS IS THE KEY**
Transform generator so first K columns ≈ identity matrix
- Row operations: XOR (GF(2) addition)
- Goal: Systematic form for fast encoding

### 4. Test Candidates
- **Order-0:** Use hard decisions directly
- **Order-1:** Flip single unreliable bits
- **Order-2:** Flip pairs of unreliable bits

### 5. Validate with CRC
Only return messages that pass CRC14 check

## Why This Works

**Without GF(2) Gaussian Elimination:**
- Flipping bits doesn't maintain LDPC constraints
- Most candidates are invalid codewords
- CRC rejects everything → 0 recoveries

**With GF(2) Gaussian Elimination:**
- Generator matrix in systematic form
- Flipping information bits → valid codewords
- Testing unreliable bits finds correct patterns
- Expected: 8-15 additional decodes

## Implementation Complexity

| Component | Lines of Code | Complexity |
|-----------|--------------|------------|
| GF(2) Gaussian Elimination | ~80 | Medium |
| Generator matrix cache | ~40 | Easy |
| Fast RREF encoding | ~30 | Easy |
| Order-0 decode | ~50 | Easy |
| Order-1 decode | ~40 | Easy |
| Order-2 decode | ~50 | Medium |
| **Total** | **~290** | **Medium** |

## Expected Impact

```
Current:    6/22 messages (27%)
+ Order-0:  11-16/22 messages (50-73%)
+ Order-1:  14-21/22 messages (64-95%)
+ Order-2:  15-22/22 messages (68-100%)
```

## Time Budget

- Infrastructure & GF(2): 4-6 hours
- Order-0: 2-3 hours
- Order-1: 2-3 hours
- Order-2: 2-3 hours
- Testing & debug: 4-6 hours
- **Total: 14-21 hours (2-3 days)**

## References

- WSJT-X source: `wsjtx/wsjtx-2.7.0/src/wsjtx/lib/ft8/osd174_91.f90`
- Detailed plan: `docs/OSD_IMPLEMENTATION_PLAN.md`
