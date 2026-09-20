//! Independent 4d linear map for COSY `CML .1 .1` after `RPP 100`.
//!
//! Not a copy of COSY RK8: uniform classical RK4 / midpoint on the paraxial
//! Glaser ODE, then the exact `DLACT(-5e4*D)` sandwich from `DSDED`.
//!
//! Field: `Bz = B / (1+(s/D)^2)`, `Bx = -x/2 Bz'`, `By = -y/2 Bz'`
//! (COSY `NSDP=7`, `MPOL=20`, `PMM(20)=1`).

fn chim_proton_100mev() -> f64 {
    let amu: f64 = 1.66053873e-27;
    let ezero: f64 = 1.602176462e-19;
    let clight: f64 = 2.99792458e8;
    let amumev = amu * clight * clight / ezero * 1e-6;
    let e0: f64 = 100.0;
    let m0: f64 = 1.00727646688;
    let z0: f64 = 1.0;
    let eta = e0 / m0 / amumev;
    let phi = (eta * (2.0 + eta)).sqrt();

    (amu * clight / ezero) * m0 / z0 * phi
}

fn bz(s: f64, b: f64, d: f64) -> f64 {
    b / (1.0 + (s / d) * (s / d))
}

fn dbz_ds(s: f64, b: f64, d: f64) -> f64 {
    let u = s / d;
    b * (-2.0 * s / (d * d)) / (1.0 + u * u).powi(2)
}

/// A(s) such that dU/ds = A U, U = (x, a, y, b).
fn a_matrix(s: f64, b: f64, d: f64, k: f64) -> [[f64; 4]; 4] {
    let bz = bz(s, b, d);
    let bp = dbz_ds(s, b, d);
    let mut a = [[0.0; 4]; 4];
    a[0][1] = 1.0;
    a[1][2] = k * bp / 2.0;
    a[1][3] = k * bz;
    a[2][3] = 1.0;
    a[3][0] = -k * bp / 2.0;
    a[3][1] = -k * bz;
    a
}

fn mul_a(a: &[[f64; 4]; 4], u: &[f64; 4]) -> [f64; 4] {
    let mut o = [0.0; 4];
    for i in 0..4 {
        for j in 0..4 {
            o[i] += a[i][j] * u[j];
        }
    }
    o
}

fn add(u: &[f64; 4], v: &[f64; 4], s: f64) -> [f64; 4] {
    [
        u[0] + s * v[0],
        u[1] + s * v[1],
        u[2] + s * v[2],
        u[3] + s * v[3],
    ]
}

fn rk4_step(s: f64, h: f64, u: [f64; 4], b: f64, d: f64, k: f64) -> [f64; 4] {
    let k1 = mul_a(&a_matrix(s, b, d, k), &u);
    let k2 = mul_a(&a_matrix(s + 0.5 * h, b, d, k), &add(&u, &k1, 0.5 * h));
    let k3 = mul_a(&a_matrix(s + 0.5 * h, b, d, k), &add(&u, &k2, 0.5 * h));
    let k4 = mul_a(&a_matrix(s + h, b, d, k), &add(&u, &k3, h));
    add(
        &u,
        &[
            k1[0] + 2.0 * k2[0] + 2.0 * k3[0] + k4[0],
            k1[1] + 2.0 * k2[1] + 2.0 * k3[1] + k4[1],
            k1[2] + 2.0 * k2[2] + 2.0 * k3[2] + k4[2],
            k1[3] + 2.0 * k2[3] + 2.0 * k3[3] + k4[3],
        ],
        h / 6.0,
    )
}

fn midpoint_step(s: f64, h: f64, u: [f64; 4], b: f64, d: f64, k: f64) -> [f64; 4] {
    let k1 = mul_a(&a_matrix(s, b, d, k), &u);
    let mid = add(&u, &k1, 0.5 * h);
    let k2 = mul_a(&a_matrix(s + 0.5 * h, b, d, k), &mid);
    add(&u, &k2, h)
}

fn integrate_phi(
    step: fn(f64, f64, [f64; 4], f64, f64, f64) -> [f64; 4],
    h: f64,
    b: f64,
    d: f64,
    k: f64,
    s0: f64,
    s1: f64,
) -> [[f64; 4]; 4] {
    let n = ((s1 - s0).abs() / h).round().max(1.0) as i32;
    let h = (s1 - s0) / f64::from(n);
    let mut cols = [[0.0; 4]; 4];
    for j in 0..4 {
        let mut u = [0.0; 4];
        u[j] = 1.0;
        let mut s = s0;
        for _ in 0..n {
            u = step(s, h, u, b, d, k);
            s += h;
        }
        for i in 0..4 {
            cols[i][j] = u[i];
        }
    }
    cols
}

fn mul_left_drift(l: f64, m: &[[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let mut o = *m;
    for j in 0..4 {
        o[0][j] = m[0][j] + l * m[1][j];
        o[2][j] = m[2][j] + l * m[3][j];
    }
    o
}

fn mul_right_drift(m: &[[f64; 4]; 4], l: f64) -> [[f64; 4]; 4] {
    let mut o = *m;
    for i in 0..4 {
        o[i][1] = m[i][0] * l + m[i][1];
        o[i][3] = m[i][2] * l + m[i][3];
    }
    o
}

fn matmul(a: &[[f64; 4]; 4], b: &[[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let mut o = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            o[i][j] = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    o
}

fn eye_err(m: &[[f64; 4]; 4]) -> f64 {
    let mut e = 0.0_f64;
    for i in 0..4 {
        for j in 0..4 {
            let want = if i == j { 1.0 } else { 0.0 };
            e = e.max((m[i][j] - want).abs());
        }
    }
    e
}

fn cml_linear(step: fn(f64, f64, [f64; 4], f64, f64, f64) -> [f64; 4], h: f64) -> [[f64; 4]; 4] {
    let b = 0.1;
    let d = 0.1;
    let ldl = 5e4 * d;
    let k = 1.0 / chim_proton_100mev();
    let phi = integrate_phi(step, h, b, d, k, -ldl, ldl);
    // DSDED: D(-LDL) ∘ Φ ∘ D(-LDL)
    mul_left_drift(-ldl, &mul_right_drift(&phi, -ldl))
}

fn cml_inverse(h: f64) -> [[f64; 4]; 4] {
    let b = 0.1;
    let d = 0.1;
    let ldl = 5e4 * d;
    let k = 1.0 / chim_proton_100mev();
    let phi = integrate_phi(rk4_step, h, b, d, k, ldl, -ldl);
    mul_left_drift(ldl, &mul_right_drift(&phi, ldl))
}

fn max_abs(a: &[[f64; 4]; 4], b: &[[f64; 4]; 4]) -> f64 {
    let mut m = 0.0_f64;
    for i in 0..4 {
        for j in 0..4 {
            m = m.max((a[i][j] - b[i][j]).abs());
        }
    }
    m
}

fn symplectic_err(m: &[[f64; 4]; 4]) -> f64 {
    // J = [[0,1,0,0],[-1,0,0,0],[0,0,0,1],[0,0,-1,0]]
    // err = ||M^T J M - J||_inf
    let mut mtjm = [[0.0; 4]; 4];
    let jmul = |u: usize, v: usize| -> f64 {
        match (u, v) {
            (0, 1) => 1.0,
            (1, 0) => -1.0,
            (2, 3) => 1.0,
            (3, 2) => -1.0,
            _ => 0.0,
        }
    };
    for i in 0..4 {
        for j in 0..4 {
            let mut s = 0.0;
            for k in 0..4 {
                for l in 0..4 {
                    s += m[k][i] * jmul(k, l) * m[l][j];
                }
            }
            mtjm[i][j] = s;
        }
    }
    let mut e = 0.0_f64;
    for i in 0..4 {
        for j in 0..4 {
            e = e.max((mtjm[i][j] - jmul(i, j)).abs());
        }
    }
    e
}

fn dump(name: &str, m: &[[f64; 4]; 4]) {
    eprintln!("{name}:");
    for row in m {
        eprintln!(
            "  {:.16e} {:.16e} {:.16e} {:.16e}",
            row[0], row[1], row[2], row[3]
        );
    }
}

#[test]
fn glaser_linear_map_converges_and_is_symplectic() {
    let rk_coarse = cml_linear(rk4_step, 0.02);
    let rk_fine = cml_linear(rk4_step, 0.005);
    let mid = cml_linear(midpoint_step, 0.005);
    dump("rk4 coarse", &rk_coarse);
    dump("rk4 fine", &rk_fine);
    dump("midpoint fine", &mid);
    let dh = max_abs(&rk_coarse, &rk_fine);
    let dmethod = max_abs(&rk_fine, &mid);
    eprintln!("rk4 h-refine max|Δ| = {dh:e}");
    eprintln!("rk4 vs midpoint max|Δ| = {dmethod:e}");
    let rt = eye_err(&matmul(&cml_inverse(0.005), &rk_fine));
    eprintln!("symplectic inf-err rk_fine = {:e}", symplectic_err(&rk_fine));
    eprintln!("M_inv ∘ M vs I = {rt:e}");
    assert!(dh < 2e-7, "rk4 did not stabilize: {dh:e}");
    assert!(dmethod < 2e-7, "rk4 and midpoint disagree by {dmethod:e}");
    assert!(
        symplectic_err(&rk_fine) < 1e-6,
        "linear map is not symplectic: {:e}",
        symplectic_err(&rk_fine)
    );
    assert!(rt < 1e-6, "forward+reverse field map is not identity: {rt:e}");

    // Large terms (~1 and ~0.01). COSY MAP(1) from metis; rosy from this
    // tree after the NORM __loc_I fix. ESET 1e-10.
    let cosy_row0 = [
        0.9999439058195534,
        -0.2646551365614869e-5,
        0.1059182054174401e-1,
        0.1410053506845088e-5,
    ];
    let rosy_row0 = [
        0.9999439058195464,
        -0.2646517714310903e-5,
        0.1059182054174315e-1,
        0.1410057834050349e-5,
    ];
    eprintln!("i j          ref            |rosy-ref|     |cosy-ref|");
    for j in 0..4 {
        let r = rk_fine[0][j];
        let er = (rosy_row0[j] - r).abs();
        let ec = (cosy_row0[j] - r).abs();
        eprintln!("1 {}  {r:.16e}  {er:.3e}  {ec:.3e}", j + 1);
    }
    let e_rosy_11 = (rosy_row0[0] - rk_fine[0][0]).abs();
    let e_cosy_11 = (cosy_row0[0] - rk_fine[0][0]).abs();
    let e_rosy_13 = (rosy_row0[2] - rk_fine[0][2]).abs();
    let e_cosy_13 = (cosy_row0[2] - rk_fine[0][2]).abs();
    assert!(e_rosy_11 < 2e-9 && e_cosy_11 < 2e-9);
    assert!(e_rosy_13 < 2e-9 && e_cosy_13 < 2e-9);
    let rosy_m21 = -0.1785528309985947e-3;
    let rosy_m24 = 0.1059196963522664e-1;
    assert!((rosy_m21 - rk_fine[1][0]).abs() < 2e-9);
    assert!((rosy_m24 - rk_fine[1][3]).abs() < 2e-9);
    // M12/M14 ~1e-6: schedule leftovers, not ranked.
}
