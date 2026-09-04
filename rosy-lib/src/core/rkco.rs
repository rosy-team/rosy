//! # RKCO Runtime Helper
//!
//! Sets the coefficient arrays used in the COSY eighth-order Runge-Kutta integrator.
//!
//! The 8th-order method is COSY's Kübler / Prince-Dormand 8(7) tableau.
//! Five output arrays are populated with the Butcher-tableau coefficients:
//!
//! - `c`  — nodes (stage time fractions, 13 values)
//! - `b`  — weights for the 8th-order solution (13 values)
//! - `e`  — error-estimate weights (13 values; difference between 8th- and 5th-order)
//! - `a1` — first half of the coupling matrix A (rows 2..7, flattened, 21 values)
//! - `a2` — second half of the coupling matrix A (rows 8..13, flattened, 57 values)
//!
//! ## Returns
//! All five arrays as `Vec<f64>`.

use anyhow::Result;

/// COSY `RKCO HSQR A B C D` layout used by the fox RK procedure:
/// - `hsqr` — step-size exponent (1/9)
/// - `a` — 13 nodes
/// - `b` — 13×13 coupling (only strictly lower triangle used)
/// - `c` — 8th-order weights
/// - `d` — embedded weights so `(c-d)` is the error estimate
pub fn rosy_rkco_cosy() -> Result<(f64, Vec<f64>, Vec<Vec<f64>>, Vec<f64>, Vec<f64>)> {
    // Kübler / Prince-Dormand 8(7) as filled by COSY INFINITY's RKCO
    // (and libcosy RK_INIT_COEFFS). Not Hairer DOP853.
    let a = vec![
        0.0,
        1.0 / 18.0,
        1.0 / 12.0,
        1.0 / 8.0,
        5.0 / 16.0,
        3.0 / 8.0,
        59.0 / 400.0,
        93.0 / 200.0,
        5_490_023_248.0 / 9_719_169_821.0,
        13.0 / 20.0,
        1_201_146_811.0 / 1_299_019_798.0,
        1.0,
        1.0,
    ];
    let mut b = vec![vec![0.0; 13]; 13];
    let set = |b: &mut [Vec<f64>], i: usize, j: usize, v: f64| {
        b[i - 1][j - 1] = v;
    };
    set(&mut b, 2, 1, 1.0 / 18.0);
    set(&mut b, 3, 1, 1.0 / 48.0);
    set(&mut b, 3, 2, 1.0 / 16.0);
    set(&mut b, 4, 1, 1.0 / 32.0);
    set(&mut b, 4, 3, 3.0 / 32.0);
    set(&mut b, 5, 1, 5.0 / 16.0);
    set(&mut b, 5, 3, -75.0 / 64.0);
    set(&mut b, 5, 4, 75.0 / 64.0);
    set(&mut b, 6, 1, 3.0 / 80.0);
    set(&mut b, 6, 4, 3.0 / 16.0);
    set(&mut b, 6, 5, 3.0 / 20.0);
    set(&mut b, 7, 1, 29_443_841.0 / 614_563_906.0);
    set(&mut b, 7, 4, 77_736_538.0 / 692_538_347.0);
    set(&mut b, 7, 5, -28_693_883.0 / 1_125_000_000.0);
    set(&mut b, 7, 6, 23_124_283.0 / 1_800_000_000.0);
    set(&mut b, 8, 1, 16_016_141.0 / 946_692_911.0);
    set(&mut b, 8, 4, 61_564_180.0 / 158_732_637.0);
    set(&mut b, 8, 5, 22_789_713.0 / 633_445_777.0);
    set(&mut b, 8, 6, 545_815_736.0 / 2_771_057_229.0);
    set(&mut b, 8, 7, -180_193_667.0 / 1_043_307_555.0);
    set(&mut b, 9, 1, 39_632_708.0 / 573_591_083.0);
    set(&mut b, 9, 4, -433_636_366.0 / 683_701_615.0);
    set(&mut b, 9, 5, -421_739_975.0 / 2_616_292_301.0);
    set(&mut b, 9, 6, 100_302_831.0 / 723_423_059.0);
    set(&mut b, 9, 7, 790_204_164.0 / 839_813_087.0);
    set(&mut b, 9, 8, 800_635_310.0 / 3_783_071_287.0);
    set(&mut b, 10, 1, 246_121_993.0 / 1_340_847_787.0);
    set(&mut b, 10, 4, -37_695_042_795.0 / 15_268_766_246.0);
    set(&mut b, 10, 5, -309_121_744.0 / 1_061_227_803.0);
    set(&mut b, 10, 6, -12_992_083.0 / 490_766_935.0);
    set(&mut b, 10, 7, 6_005_943_493.0 / 2_108_947_869.0);
    set(&mut b, 10, 8, 393_006_217.0 / 1_396_673_457.0);
    set(&mut b, 10, 9, 123_872_331.0 / 1_001_029_789.0);
    set(&mut b, 11, 1, -1_028_468_189.0 / 846_180_014.0);
    set(&mut b, 11, 4, 8_478_235_783.0 / 508_512_852.0);
    set(&mut b, 11, 5, 1_311_729_495.0 / 1_432_422_823.0);
    set(&mut b, 11, 6, -10_304_129_995.0 / 1_701_304_382.0);
    set(&mut b, 11, 7, -48_777_925_059.0 / 3_047_939_560.0);
    set(&mut b, 11, 8, 15_336_726_248.0 / 1_032_824_649.0);
    set(&mut b, 11, 9, -45_442_868_181.0 / 3_398_467_696.0);
    set(&mut b, 11, 10, 3_065_993_473.0 / 597_172_653.0);
    set(&mut b, 12, 1, 185_892_177.0 / 718_116_043.0);
    set(&mut b, 12, 4, -3_185_094_517.0 / 667_107_341.0);
    set(&mut b, 12, 5, -477_755_414.0 / 1_098_053_517.0);
    set(&mut b, 12, 6, -703_635_378.0 / 230_739_211.0);
    set(&mut b, 12, 7, 5_731_566_787.0 / 1_027_545_527.0);
    set(&mut b, 12, 8, 5_232_866_602.0 / 850_066_563.0);
    set(&mut b, 12, 9, -4_093_664_535.0 / 808_688_257.0);
    set(&mut b, 12, 10, 3_962_137_247.0 / 1_805_957_418.0);
    set(&mut b, 12, 11, 65_686_358.0 / 487_910_083.0);
    set(&mut b, 13, 1, 403_863_854.0 / 491_063_109.0);
    set(&mut b, 13, 4, -5_068_492_393.0 / 434_740_067.0);
    set(&mut b, 13, 5, -411_421_997.0 / 543_043_805.0);
    set(&mut b, 13, 6, 652_783_627.0 / 914_296_604.0);
    set(&mut b, 13, 7, 11_173_962_825.0 / 925_320_556.0);
    set(&mut b, 13, 8, -13_158_990_841.0 / 6_184_727_034.0);
    set(&mut b, 13, 9, 3_936_647_629.0 / 1_978_049_680.0);
    set(&mut b, 13, 10, -160_528_059.0 / 685_178_525.0);
    set(&mut b, 13, 11, 248_638_103.0 / 1_413_531_060.0);

    let mut c = vec![0.0; 13];
    c[0] = 14_005_451.0 / 335_480_064.0;
    c[5] = -59_238_493.0 / 1_068_277_825.0;
    c[6] = 181_606_767.0 / 758_867_731.0;
    c[7] = 561_292_985.0 / 797_845_732.0;
    c[8] = -1_041_891_430.0 / 1_371_343_529.0;
    c[9] = 760_417_239.0 / 1_151_165_299.0;
    c[10] = 118_820_643.0 / 751_138_087.0;
    c[11] = -528_747_749.0 / 2_220_607_170.0;
    c[12] = 0.25;

    let mut d = vec![0.0; 13];
    d[0] = 13_451_932.0 / 455_176_623.0;
    d[5] = -808_719_846.0 / 976_000_145.0;
    d[6] = 1_757_004_468.0 / 5_645_159_321.0;
    d[7] = 656_045_339.0 / 265_891_186.0;
    d[8] = -3_867_574_721.0 / 1_518_517_206.0;
    d[9] = 465_885_868.0 / 322_736_535.0;
    d[10] = 53_011_238.0 / 667_516_719.0;
    d[11] = 2.0 / 45.0;
    d[12] = 0.0;

    Ok((1.0 / 9.0, a, b, c, d))
}

/// Populate the five Runge-Kutta coefficient arrays.
///
/// Packs the COSY Kübler 8(7) tableau (`rosy_rkco_cosy`) into the
/// `RKCO C B E A1 A2` layout: nodes, 8th-order weights, `C-D` error
/// weights, then the flattened lower triangle of the coupling matrix.
///
/// Returns `(c, b, e, a1, a2)`.
pub fn rosy_rkco() -> Result<(Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>)> {
    let (_hsqr, nodes, bmat, w8, w7) = rosy_rkco_cosy()?;
    let e: Vec<f64> = w8.iter().zip(w7.iter()).map(|(c, d)| c - d).collect();
    let mut a1 = Vec::with_capacity(21);
    for j in 1..7 {
        for k in 0..j {
            a1.push(bmat[j][k]);
        }
    }
    let mut a2 = Vec::with_capacity(57);
    for j in 7..13 {
        for k in 0..j {
            a2.push(bmat[j][k]);
        }
    }
    Ok((nodes, w8, e, a1, a2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosy_layout_hsqr_and_nodes() {
        let (hsqr, a, b, c, d) = rosy_rkco_cosy().unwrap();
        assert!((hsqr - 1.0 / 9.0).abs() < 1e-15);
        assert_eq!(a.len(), 13);
        assert_eq!(b.len(), 13);
        assert_eq!(b[0].len(), 13);
        assert_eq!(c.len(), 13);
        assert_eq!(d.len(), 13);
        assert!(a[0].abs() < 1e-15);
        assert!((a[1] - 1.0 / 18.0).abs() < 1e-15);
        assert!((b[1][0] - 1.0 / 18.0).abs() < 1e-15);
        assert!(d[12].abs() < 1e-15);
        assert!((c[12] - d[12]).abs() > 1e-9);
        let wsum: f64 = c.iter().sum();
        assert!((wsum - 1.0).abs() < 1e-9);
    }

    #[test]
    fn five_array_pack_matches_cosy_nodes_and_weights() {
        let (c, b, e, a1, a2) = rosy_rkco().unwrap();
        let (_, nodes, bmat, w8, w7) = rosy_rkco_cosy().unwrap();
        assert_eq!(c, nodes);
        assert_eq!(b, w8);
        assert_eq!(a1.len(), 21);
        assert_eq!(a2.len(), 57);
        assert!((e[0] - (w8[0] - w7[0])).abs() < 1e-15);
        assert!((a1[0] - bmat[1][0]).abs() < 1e-15);
    }
}
