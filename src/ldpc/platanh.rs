//! Piecewise linear atanh approximation from WSJT-X

/// Piecewise linear approximation of atanh used by WSJT-X
///
/// This is NOT the mathematical atanh function! WSJT-X uses a piecewise
/// linear approximation that has been tuned for LDPC decoding performance.
/// The approximation differs by 10-40% from mathematical atanh in typical
/// operating ranges, and caps output at ±7.0 for numerical stability.
///
/// The function uses 5 linear segments:
/// - |x| ≤ 0.664: y = x / 0.83
/// - 0.664 < |x| ≤ 0.9217: y = sign(x) * (|x| - 0.4064) / 0.322
/// - 0.9217 < |x| ≤ 0.9951: y = sign(x) * (|x| - 0.8378) / 0.0524
/// - 0.9951 < |x| ≤ 0.9998: y = sign(x) * (|x| - 0.9914) / 0.0012
/// - |x| > 0.9998: y = sign(x) * 7.0
///
/// Reference: wsjtx/lib/platanh.f90
#[inline]
pub(super) fn platanh(x: f32) -> f32 {
    let isign = if x < 0.0 { -1.0 } else { 1.0 };
    let z = x.abs();

    if z <= 0.664 {
        x / 0.83
    } else if z <= 0.9217 {
        isign * (z - 0.4064) / 0.322
    } else if z <= 0.9951 {
        isign * (z - 0.8378) / 0.0524
    } else if z <= 0.9998 {
        isign * (z - 0.9914) / 0.0012
    } else {
        isign * 7.0
    }
}
