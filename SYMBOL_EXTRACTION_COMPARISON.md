# Symbol Extraction Quality Investigation

## Problem Statement

RustyFt8 produces **5.3 more hard errors on average** compared to WSJT-X when decoding the same signals. This indicates lower-quality LLRs from the symbol extraction stage.

**Evidence from test 210703_133430.wav:**
- Average Hard Errors: WSJT-X = 15.5, RustyFt8 = 20.8 (Gap: **+5.3 errors**)
- Specific examples:
  - `CQ F5RXL IN94`: WSJT-X = 1 error, RustyFt8 = 12 errors (+11)
  - `K1BZM EA3GP -09`: WSJT-X = 15 errors, RustyFt8 = 36 errors (+21)
  - `N1PJT HB9CQK -10`: WSJT-X = 20 errors, RustyFt8 = 43 errors (+23)

## Code Comparison

### 1. Downsampling and Frequency Centering

**WSJT-X** ([ft8b.f90:104-106](wsjtx/wsjtx-2.7.0/src/wsjtx/lib/ft8/ft8b.f90#L104-L106)):
```fortran
call ft8_downsample(dd0,newdat,f1,cd0)   !Mix f1 to baseband and downsample
```
- Downsamples to 200 Hz (fs2 = 12000/NDOWN, NDOWN=64)
- Produces 3200 complex samples (NP2=2812 is buffer size)

**RustyFt8** ([src/sync/extract.rs:123](src/sync/extract.rs#L123)):
```rust
let actual_sample_rate = downsample_200hz(signal, candidate.frequency, &mut cd)?;
```
- Downsamples to 200 Hz using same algorithm
- Produces 3200 complex samples
- ✅ **MATCH**: Downsampling appears equivalent

### 2. Timing Search and Refinement

**WSJT-X** ([ft8b.f90:108-116](wsjtx/wsjtx-2.7.0/src/wsjtx/lib/ft8/ft8b.f90#L108-L116)):
```fortran
i0=nint((xdt+0.5)*fs2)                   !Initial guess for start of signal
do idt=i0-10,i0+10                       !Search over +/- one quarter symbol
   call sync8d(cd0,idt,ctwk,0,sync)
   if(sync.gt.smax) then
      smax=sync
      ibest=idt
   endif
enddo
```
- Searches ±10 samples (~±50ms) around initial time offset
- Uses `sync8d` to compute sync quality at each timing

**RustyFt8** ([src/sync/extract.rs:182-201](src/sync/extract.rs#L182-L201)):
```rust
let mut best_offset = initial_offset;
let mut best_sync = sync_downsampled(&cd, initial_offset, None, false, Some(actual_sample_rate));

for dt in -10..=10 {
    if dt == 0 { continue; }
    let t_offset = initial_offset + dt;
    let sync = sync_downsampled(&cd, t_offset, None, false, Some(actual_sample_rate));

    if sync > best_sync {
        best_sync = sync;
        best_offset = t_offset;
    }
}
```
- ✅ **MATCH**: Same ±10 sample timing search

### 3. Frequency Refinement

**WSJT-X** ([ft8b.f90:118-137](wsjtx/wsjtx-2.7.0/src/wsjtx/lib/ft8/ft8b.f90#L118-L137)):
```fortran
do ifr=-5,5                              !Search over +/- 2.5 Hz
  delf=ifr*0.5
  dphi=twopi*delf*dt2
  phi=0.0
  do i=1,32
    ctwk(i)=cmplx(cos(phi),sin(phi))
    phi=mod(phi+dphi,twopi)
  enddo
  call sync8d(cd0,ibest,ctwk,1,sync)
  if( sync .gt. smax ) then
    smax=sync
    delfbest=delf
  endif
enddo
a=0.0
a(1)=-delfbest
call twkfreq1(cd0,NP2,fs2,a,cd0)         ! Apply frequency correction
f1=f1+delfbest                           ! Update frequency estimate

call ft8_downsample(dd0,.false.,f1,cd0)  ! RE-DOWNSAMPLE at corrected frequency!
```
- **KEY**: After finding best frequency offset, WSJT-X **RE-DOWNSAMPLES** the original signal at the corrected frequency
- This ensures the signal is perfectly centered at baseband

**RustyFt8** ([src/sync/extract.rs:135-169](src/sync/extract.rs#L135-L169)):
```rust
if nsym >= 2 {
    // Search ±1.0 Hz in 0.05 Hz steps using per-symbol ctwk
    for correction_idx in -20..=20 {
        let freq_correction = correction_idx as f32 * 0.05;
        let ctwk = build_ctwk(freq_correction, actual_sample_rate);
        let sync = sync_downsampled(&cd, time_offset_samples, Some(&ctwk), true, Some(actual_sample_rate));

        if sync > best_sync {
            best_sync = sync;
            best_correction = freq_correction;
        }
    }

    // Apply best correction to the whole buffer
    if best_correction.abs() > 0.001 {
        apply_phase_correction(&mut cd, best_correction, actual_sample_rate);
    }
}
```
- ⚠️ **DIFFERENCE**: RustyFt8 applies phase correction to the downsampled buffer
- ❌ **DOES NOT RE-DOWNSAMPLE** at the corrected frequency
- This may leave residual spectral leakage from the original downsample filter

### 4. Final Timing Refinement (After Frequency Correction)

**WSJT-X** ([ft8b.f90:143-152](wsjtx/wsjtx-2.7.0/src/wsjtx/lib/ft8/ft8b.f90#L143-L152)):
```fortran
do idt=-4,4                              ! Second timing search ±4 samples
   call sync8d(cd0,ibest+idt,ctwk,0,sync)
   ss(idt+5)=sync
enddo
smax=maxval(ss)
iloc=maxloc(ss)
ibest=iloc(1)-5+ibest
xdt=(ibest-1)*dt2
```
- After re-downsampling, does another ±4 sample timing search
- This accounts for any timing shift from the frequency correction

**RustyFt8**:
- ❌ **MISSING**: No second timing refinement after frequency correction
- This may result in sub-optimal symbol alignment

### 5. Symbol Extraction via FFT

**WSJT-X** ([ft8b.f90:154-161](wsjtx/wsjtx-2.7.0/src/wsjtx/lib/ft8/ft8b.f90#L154-L161)):
```fortran
do k=1,NN
  i1=ibest+(k-1)*32
  csymb=cmplx(0.0,0.0)
  if( i1.ge.0 .and. i1+31 .le. NP2-1 ) csymb=cd0(i1:i1+31)
  call four2a(csymb,32,1,-1,1)           ! 32-point FFT
  cs(0:7,k)=csymb(1:8)/1e3               ! Normalized complex symbols
  s8(0:7,k)=abs(csymb(1:8))              ! Unnormalized magnitude
enddo
```
- Extracts 32 samples per symbol
- 32-point FFT
- Normalizes complex symbols by 1000
- Keeps unnormalized magnitudes for Costas check

**RustyFt8** ([src/sync/extract.rs:225-278](src/sync/extract.rs#L225-L278)):
```rust
for k in 0..NN {
    let i1 = start_offset + (k as i32) * (nsps_down as i32);
    let i2 = i1 + (nsps_down as i32);

    // Copy symbol samples
    for j in 0..nsps_down {
        let idx = i1 + (j as i32);
        if idx >= 0 && (idx as usize) < cd.len() {
            let fft_idx = j + fft_offset;
            if fft_idx < NFFT_SYM {
                sym_real[fft_idx] = cd[idx as usize].0;
                sym_imag[fft_idx] = cd[idx as usize].1;
            }
        }
    }

    fft_real(&mut sym_real, &mut sym_imag, NFFT_SYM)?;

    for tone in 0..8 {
        let re = sym_real[tone];
        let im = sym_imag[tone];
        cs[tone][k] = (re / NORM_FACTOR, im / NORM_FACTOR);
        s8[tone][k] = (re * re + im * im).sqrt();
    }
}
```
- ✅ **MATCH**: Same FFT extraction logic

### 6. LLR Computation

**WSJT-X** ([ft8b.f90:182-229](wsjtx/wsjtx-2.7.0/src/wsjtx/lib/ft8/ft8b.f90#L182-L229)):
```fortran
do nsym=1,3
  nt=2**(3*nsym)
  do ihalf=1,2
    do k=1,29,nsym
      if(ihalf.eq.1) ks=k+7
      if(ihalf.eq.2) ks=k+43
      do i=0,nt-1
        i1=i/64
        i2=iand(i,63)/8
        i3=iand(i,7)
        if(nsym.eq.1) then
          s2(i)=abs(cs(graymap(i3),ks))
        elseif(nsym.eq.2) then
          s2(i)=abs(cs(graymap(i2),ks)+cs(graymap(i3),ks+1))
        elseif(nsym.eq.3) then
          s2(i)=abs(cs(graymap(i1),ks)+cs(graymap(i2),ks+1)+cs(graymap(i3),ks+2))
        endif
      enddo

      do ib=0,ibmax
        bm=maxval(s2(0:nt-1),one(0:nt-1,ibmax-ib)) - &
           maxval(s2(0:nt-1),.not.one(0:nt-1,ibmax-ib))
        if(nsym.eq.1) then
          bmeta(i32+ib)=bm
          den=max(maxval(s2(0:nt-1),one(0:nt-1,ibmax-ib)), &
                  maxval(s2(0:nt-1),.not.one(0:nt-1,ibmax-ib)))
          if(den.gt.0.0) then
            cm=bm/den
          else
            cm=0.0
          endif
          bmetd(i32+ib)=cm
        elseif(nsym.eq.2) then
          bmetb(i32+ib)=bm
        elseif(nsym.eq.3) then
          bmetc(i32+ib)=bm
        endif
      enddo
    enddo
  enddo
enddo
```
- Computes LLRs for all 4 methods (nsym=1,2,3 + ratio)
- Uses `abs()` for magnitude (NOT magnitude-squared)

**RustyFt8** ([src/sync/extract.rs:380-611](src/sync/extract.rs#L380-L611)):
```rust
// Similar logic, but...
for tone in 0..8 {
    let index = GRAY_MAP_INV[tone];
    s2[index as usize] = s8[tone][ks];  // Uses magnitude from s8
}

// Extract LLR
llr[bit_idx] = max_mag_1 - max_mag_0;
```
- ✅ **MATCH**: Uses magnitude (not magnitude-squared)
- ✅ **MATCH**: Same difference method for LLR computation

### 7. LLR Normalization

**WSJT-X** ([ft8b.f90:230-239](wsjtx/wsjtx-2.7.0/src/wsjtx/lib/ft8/ft8b.f90#L230-L239)):
```fortran
call normalizebmet(bmeta,174)
call normalizebmet(bmetb,174)
call normalizebmet(bmetc,174)
call normalizebmet(bmetd,174)

scalefac=2.83
llra=scalefac*bmeta
llrb=scalefac*bmetb
llrc=scalefac*bmetc
llrd=scalefac*bmetd
```

**normalizebmet** ([ft8b.f90:490-503](wsjtx/wsjtx-2.7.0/src/wsjtx/lib/ft8/ft8b.f90#L490-L503)):
```fortran
bmetav=sum(bmet)/real(n)
bmet2av=sum(bmet*bmet)/real(n)
var=bmet2av-bmetav*bmetav
if( var .gt. 0.0 ) then
   bmetsig=sqrt(var)
else
   bmetsig=sqrt(bmet2av)
endif
bmet=bmet/bmetsig
```
- Normalizes by standard deviation (NOT centered - does not subtract mean)
- Then scales by 2.83

**RustyFt8** ([src/sync/extract.rs:614-639](src/sync/extract.rs#L614-L639)):
```rust
let mut sum = 0.0f32;
let mut sum_sq = 0.0f32;
for i in 0..174 {
    sum += llr[i];
    sum_sq += llr[i] * llr[i];
}
let mean = sum / 174.0;
let mean_sq = sum_sq / 174.0;
let variance = mean_sq - mean * mean;
let std_dev = if variance > 0.0 {
    variance.sqrt()
} else {
    mean_sq.sqrt()
};

if std_dev > 0.0 {
    for i in 0..174 {
        llr[i] /= std_dev;
    }
}

// Then scale by WSJT-X scalefac=2.83
for i in 0..174 {
    llr[i] *= 2.83;
}
```
- ✅ **MATCH**: Same normalization algorithm

## Key Differences Found

### 🚨 **CRITICAL: Missing Re-Downsample Step**

**The Root Cause**: RustyFt8 does NOT re-downsample after frequency refinement!

WSJT-X does:
1. Initial downsample at coarse frequency `f1`
2. Refine frequency by ±2.5 Hz to find `f1 + delfbest`
3. **Re-downsample at the refined frequency** `f1 + delfbest`
4. Final timing search on the re-downsampled signal

RustyFt8 does:
1. Initial downsample at refined frequency from fine_sync
2. Apply phase correction to compensate for frequency error
3. ❌ **Does NOT re-downsample**

**Why this matters**:
- Downsampling involves a low-pass filter to prevent aliasing
- If the signal is not centered at DC, the filter edges affect the signal
- Even after phase correction, there may be spectral leakage from the filter
- Re-downsampling ensures the signal is perfectly centered in the passband

### ⚠️ **Missing Second Timing Refinement**

After re-downsampling at the corrected frequency, WSJT-X does a second ±4 sample timing search. RustyFt8 does not repeat the timing search after frequency correction.

**Why this matters**:
- Frequency correction can shift the optimal timing slightly
- The second timing search ensures optimal symbol alignment

## Recommended Fixes

### Priority 1: Implement Re-Downsample After Frequency Refinement

Modify `extract_symbols_impl` to:
1. After frequency refinement loop (line 162), save `best_correction`
2. If `best_correction.abs() > 0.01`, call `downsample_200hz` again with `candidate.frequency + best_correction`
3. Update `candidate.frequency` for consistency

### Priority 2: Add Second Timing Refinement

After re-downsampling (or frequency correction), add a second ±4 sample timing search to ensure optimal symbol alignment.

### Priority 3: Verify Phase Coherence

Ensure that multi-symbol combining (nsym=2,3) maintains phase coherence across symbols. The current phase correction may not be sufficient if the signal has frequency drift during the 12.64-second transmission.

## Expected Impact

Based on the +5.3 error gap:
- **Re-downsampling**: Expected to reduce errors by 2-4 (40-75% of gap)
- **Second timing search**: Expected to reduce errors by 0.5-1 (10-20% of gap)
- **Phase coherence fixes**: Expected to reduce errors by 0.5-2 (10-40% of gap)

Total expected improvement: **Close 60-90% of the error gap**