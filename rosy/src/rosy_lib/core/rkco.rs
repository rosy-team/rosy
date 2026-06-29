//! # RKCO Runtime Helper
//!
//! Sets the coefficient arrays used in the COSY eighth-order Runge-Kutta integrator.
//!
//! The 8th-order method is based on the Dormand-Prince DOP853 scheme (Hairer, Norsett,
//! Wanner). Five output arrays are populated with the Butcher-tableau coefficients:
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

/// Populate the five Runge-Kutta coefficient arrays for the DOP853 integrator.
///
/// Returns `(c, b, e, a1, a2)`.
pub fn rosy_rkco() -> Result<(Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>)> {
    // --- c: node coefficients (c[0] = 0, then c[1..12]) ---
    // DOP853 nodes (Hairer et al., "Solving ODEs I", 2nd ed., Table II.6.2)
    let c: Vec<f64> = vec![
        0.0,
        0.526_001_519_587_677_e-1,
        0.789_002_279_381_516_e-1,
        0.118_350_341_907_227,
        0.281_649_658_092_772_7,
        0.333_333_333_333_333_3,
        0.25,
        0.307_692_307_692_307_7,
        0.651_282_051_282_051_3,
        0.6,
        0.857_142_857_142_857,
        1.0,
        1.0,
    ];

    // --- b: 8th-order weights (Hairer, Nørsett, Wanner "Solving ODEs I", dop853.f) ---
    // Sum = 1.0 exactly; b[1..4] and b[12] = 0 (unused stages)
    let b: Vec<f64> = vec![
        5.42937341165687296e-2,
        0.0,
        0.0,
        0.0,
        0.0,
        4.45031289275240888e+0,
        1.89151789931450038e+0,
        -5.80120396001058478e+0,
        3.11164366957819894e-1,
        -1.52160949662516079e-1,
        2.01365400804030348e-1,
        4.47106157277725905e-2,
        0.0,
    ];

    // --- e: error-estimate coefficients (b8 - b5 differences) ---
    // These are the differences between 8th- and embedded 5th-order weights.
    let e: Vec<f64> = vec![
        0.1312004499419488073e-1,
        0.0,
        0.0,
        0.0,
        0.0,
        -0.1225156446376204440e-1,
        -0.4957589496572501915e-1,
        0.1664377182454986536e+0,
        -0.3558496486701148929e+0,
        0.9340847839611065608e+0,
        0.4347950448516186963e+0,
        0.1061055815394039673e+1,
        0.0,
    ];

    // --- a1: Butcher tableau rows 2..7 (lower-triangular A, flattened) ---
    // Row 2 (1 element), row 3 (2), row 4 (3), row 5 (4), row 6 (5), row 7 (6) = 21 values
    let a1: Vec<f64> = vec![
        // row 2
        5.26001519587677318e-2,
        // row 3
        1.97250569845378994e-2,
        5.91751709536136983e-2,
        // row 4
        2.95875854768068491e-2,
        0.0,
        8.87627564304205475e-2,
        // row 5
        2.41365641823501286e-1,
        0.0,
        -8.84549479328286076e-1,
        9.24834003261792003e-1,
        // row 6
        3.7037037037037037e-2,
        0.0,
        0.0,
        1.70828608729473871e-1,
        1.25467687566822428e-1,
        // row 7
        3.7109375e-2,
        0.0,
        0.0,
        1.70252211019544040e-1,
        6.02165389804559092e-2,
        -1.7578125e-2,
    ];

    // --- a2: Butcher tableau rows 8..13 (6 rows: 7,8,9,10,11,12 elements) ---
    // Rows 8..13 have 7+8+9+10+11+12 = 57 values total.
    let a2: Vec<f64> = vec![
        // row 8 (7 elements)
        3.70920001185047927e-2,
        0.0,
        0.0,
        1.70383925712239993e-1,
        1.07262030446373284e-1,
        -1.53194377486244882e-2,
        8.27378916792996988e-3,
        // row 9 (8 elements)
        6.24110958716075717e-1,
        0.0,
        0.0,
        -3.36089262944694129e0,
        -8.68219346841726006e-1,
        2.72075314366958199e1,
        2.01540675504778934e1,
        -4.34898841810699588e1,
        // row 10 (9 elements)
        4.77662536438264366e-1,
        0.0,
        0.0,
        -2.48811461997166764e0,
        -5.90290826836842996e-1,
        2.12300514481811942e1,
        2.22347739612513272e1,
        -2.92484766483039292e1,
        -2.91669980647368260e0,
        // row 11 (10 elements)
        -9.31463719476595947e-1,
        0.0,
        0.0,
        5.64841697574841975e0,
        7.36446505087717503e-1,
        -2.66558266064889469e1,
        -2.82741701610524682e1,
        3.34291956757620551e1,
        2.86516118900552873e0,
        1.14095661016660820e1,
        // row 12 (11 elements)
        2.27331014751653821e-1,
        0.0,
        0.0,
        -1.05344954667372501e0,
        -2.00087205822486249e-2,
        1.57982909820588250e1,
        2.57112430717927171e1,
        -4.05313840176771403e1,
        -1.37316482655824625e1,
        2.13374040065074902e1,
        2.93930402093266800e0,
        // row 13 (12 elements) — FSAL: mirrors b weights exactly
        5.42937341165687296e-2,
        0.0,
        0.0,
        0.0,
        0.0,
        4.45031289275240888e+0,
        1.89151789931450038e+0,
        -5.80120396001058478e+0,
        3.11164366957819894e-1,
        -1.52160949662516079e-1,
        2.01365400804030348e-1,
        4.47106157277725905e-2,
    ];

    Ok((c, b, e, a1, a2))
}
