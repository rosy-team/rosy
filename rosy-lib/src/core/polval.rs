//! POLVAL - polynomial evaluation / composition.
//!
//! `POLVAL L P NP A NA R NR;`
//!
//! Lets the polynomial described by NP DA vectors stored in the array P
//! act on the NA arguments A, and stores the NR results in R.
//!
//! L == 1 is Horner evaluation (the COSY default). L >= 100 is COSY's
//! "prepare evaluation trees, do not evaluate" (TR uses `POLVAL 105`
//! for this). Negative L means "evaluate using a previously prepared
//! tree"; Rosy just evaluates, since Horner is rebuilt from the DAs.
//! Rosy always uses a Horner factorization of the monomial addressing:
//! each monomial is `parent * x_v`, so real / particle evaluation and
//! DA/CD composition share intermediate products instead of powering
//! every term from scratch.

use crate::RosyValue;
use crate::taylor::{CD, DA};
use anyhow::{Result, bail};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[cfg(feature = "nightly-simd")]
use std::simd::StdFloat;
#[cfg(feature = "nightly-simd")]
use std::simd::prelude::*;

#[cfg(feature = "nightly-simd")]
const LANES: usize = 4;

/// Evaluate NP polynomials (stored in `p_array` as DA vectors) at the NA real
/// arguments in `a_array`, writing NR results into `r_array`.
///
/// # Arguments
/// * `l`        - evaluation mode (1 = Horner; >= 100 = tree prep, no eval)
/// * `p_array`  - slice of NP DA polynomials
/// * `np`       - number of polynomials to evaluate
/// * `a_array`  - slice of NA real-valued arguments
/// * `na`       - number of arguments
/// * `r_array`  - output vector, must be large enough to hold NR results
/// * `nr`       - number of results to write
pub trait PolvalAnySrc {
    fn polval_any_cells(&self) -> Vec<RosyValue>;
}
impl PolvalAnySrc for Vec<RosyValue> {
    fn polval_any_cells(&self) -> Vec<RosyValue> {
        self.clone()
    }
}
impl PolvalAnySrc for RosyValue {
    fn polval_any_cells(&self) -> Vec<RosyValue> {
        match self {
            RosyValue::Arr(v) => v.clone(),
            other => vec![other.clone()],
        }
    }
}
impl PolvalAnySrc for [RosyValue] {
    fn polval_any_cells(&self) -> Vec<RosyValue> {
        self.to_vec()
    }
}
impl PolvalAnySrc for Vec<DA> {
    fn polval_any_cells(&self) -> Vec<RosyValue> {
        self.iter().cloned().map(RosyValue::DA).collect()
    }
}
impl PolvalAnySrc for [DA] {
    fn polval_any_cells(&self) -> Vec<RosyValue> {
        self.iter().cloned().map(RosyValue::DA).collect()
    }
}
impl PolvalAnySrc for Vec<f64> {
    fn polval_any_cells(&self) -> Vec<RosyValue> {
        self.iter().copied().map(RosyValue::RE).collect()
    }
}
impl PolvalAnySrc for Vec<Vec<f64>> {
    fn polval_any_cells(&self) -> Vec<RosyValue> {
        self.iter()
            .cloned()
            .map(|col| RosyValue::Arr(col.into_iter().map(RosyValue::RE).collect()))
            .collect()
    }
}

pub trait PolvalAnyDst {
    fn store_polval_any(&mut self, v: Vec<RosyValue>) -> Result<()>;
}

/// A result that is a list of values (one per ray) cannot go into a cell
/// typed as one real or one DA. Dropping all but the first value is what hid
/// PHAD's missing far-field kicks, so refuse instead.
fn single_polval_cell(i: usize, x: &RosyValue) -> Result<()> {
    let n = match x {
        RosyValue::VE(v) => v.len(),
        RosyValue::Arr(v) => v.len(),
        _ => 1,
    };
    if n > 1 {
        anyhow::bail!(
            "POLVAL result {} has {} values, but the result array holds one value per entry",
            i + 1,
            n
        );
    }
    Ok(())
}
impl PolvalAnyDst for Vec<RosyValue> {
    fn store_polval_any(&mut self, v: Vec<RosyValue>) -> Result<()> {
        if v.len() > self.len() {
            self.resize(v.len(), RosyValue::RE(0.0));
        }
        for (i, x) in v.into_iter().enumerate() {
            self[i] = x;
        }
        Ok(())
    }
}
impl PolvalAnyDst for RosyValue {
    fn store_polval_any(&mut self, v: Vec<RosyValue>) -> Result<()> {
        if v.len() == 1 {
            *self = v.into_iter().next().unwrap();
        } else {
            *self = RosyValue::Arr(v);
        }
        Ok(())
    }
}
impl PolvalAnyDst for Vec<DA> {
    fn store_polval_any(&mut self, v: Vec<RosyValue>) -> Result<()> {
        for (i, x) in v.iter().enumerate() {
            single_polval_cell(i, x)?;
        }
        *self = v
            .into_iter()
            .map(|x| x.expect_da().unwrap_or_else(|_| DA::zero()))
            .collect();
        Ok(())
    }
}
impl PolvalAnyDst for Vec<f64> {
    fn store_polval_any(&mut self, v: Vec<RosyValue>) -> Result<()> {
        for (i, x) in v.iter().enumerate() {
            single_polval_cell(i, x)?;
        }
        *self = v.into_iter().map(|x| x.as_f64()).collect();
        Ok(())
    }
}
impl PolvalAnyDst for Vec<Vec<f64>> {
    fn store_polval_any(&mut self, v: Vec<RosyValue>) -> Result<()> {
        *self = v
            .into_iter()
            .map(|c| match c {
                RosyValue::VE(col) => col,
                RosyValue::Arr(col) => col.into_iter().map(|x| x.as_f64()).collect(),
                other => vec![other.as_f64()],
            })
            .collect();
        Ok(())
    }
}

/// COSY `POLVAL L` with L >= 100 only builds Horner trees and must not
/// write results. TR does `POLVAL 105` before the first dump, then
/// `POLVAL -5` each turn to actually push rays.
fn polval_is_tree_prep(l: f64) -> bool {
    l >= 100.0
}

fn rosy_value_is_cd(v: &RosyValue) -> bool {
    match v {
        RosyValue::CD(_) | RosyValue::CM(_) => true,
        RosyValue::Arr(xs) => xs.iter().any(rosy_value_is_cd),
        _ => false,
    }
}

/// Fox ANY arrays: pick DA compose, CD compose, particle VE, or scalar RE.
pub fn rosy_polval_any(
    l: impl crate::IntoF64,
    p_array: &(impl crate::PolvalDaSrc + PolvalAnySrc),
    np: impl crate::IntoF64,
    a_array: &impl PolvalAnySrc,
    na: impl crate::IntoF64,
    r_array: &mut impl PolvalAnyDst,
    nr: impl crate::IntoF64,
) -> Result<()> {
    let lv = l.into_f64();
    if polval_is_tree_prep(lv) {
        return Ok(());
    }
    let a_cells = a_array.polval_any_cells();
    let p_cells = p_array.polval_any_cells();
    if p_cells.iter().any(rosy_value_is_cd) || a_cells.iter().any(rosy_value_is_cd) {
        let mut out: Vec<CD> = Vec::new();
        rosy_polval_cd(lv, &p_cells, np, &a_cells, na, &mut out, nr)?;
        r_array.store_polval_any(out.into_iter().map(RosyValue::CD).collect())?;
        return Ok(());
    }
    match a_cells.first() {
        Some(RosyValue::DA(_)) => {
            let p = p_array.to_da_vec();
            let a: Vec<DA> = a_cells
                .iter()
                .map(|x| x.clone().expect_da().unwrap_or_else(|_| DA::zero()))
                .collect();
            let mut out = Vec::new();
            rosy_polval_da(
                lv,
                &p,
                crate::rosy_as_usize(&np.into_f64()),
                &a,
                crate::rosy_as_usize(&na.into_f64()),
                &mut out,
                crate::rosy_as_usize(&nr.into_f64()),
            )?;
            r_array.store_polval_any(out.into_iter().map(RosyValue::DA).collect())?;
            Ok(())
        }
        Some(RosyValue::Arr(_)) | Some(RosyValue::VE(_)) => {
            let p = p_array.to_da_vec();
            let a: Vec<Vec<f64>> = a_cells
                .iter()
                .map(|c| match c {
                    RosyValue::VE(v) => v.clone(),
                    RosyValue::Arr(v) => v.iter().map(|x| x.as_f64()).collect(),
                    other => vec![other.as_f64()],
                })
                .collect();
            let mut out: Vec<Vec<f64>> = Vec::new();
            rosy_polval_ve(
                lv,
                &p,
                crate::rosy_as_usize(&np.into_f64()),
                &a,
                crate::rosy_as_usize(&na.into_f64()),
                &mut out,
                crate::rosy_as_usize(&nr.into_f64()),
            )?;
            r_array.store_polval_any(
                out.into_iter()
                    .map(|col| RosyValue::Arr(col.into_iter().map(RosyValue::RE).collect()))
                    .collect(),
            )?;
            Ok(())
        }
        _ => {
            let mut out: Vec<RosyValue> = Vec::new();
            rosy_polval_re(lv, p_array, np, &a_cells, na, &mut out, nr)?;
            r_array.store_polval_any(out)?;
            Ok(())
        }
    }
}

pub fn rosy_polval_re(
    l: impl crate::IntoF64,
    p_array: &impl crate::PolvalDaSrc,
    np: impl crate::IntoF64,
    a_array: &(impl crate::PolvalReSrc + ?Sized),
    na: impl crate::IntoF64,
    r_array: &mut impl crate::PolvalReDst,
    nr: impl crate::IntoF64,
) -> Result<()> {
    if polval_is_tree_prep(l.into_f64()) {
        return Ok(());
    }
    let p_array = p_array.to_da_vec();
    let a_array = a_array.to_re_vec();
    let np = crate::rosy_as_usize(&np.into_f64());
    let na = crate::rosy_as_usize(&na.into_f64());
    let nr = crate::rosy_as_usize(&nr.into_f64());
    if np < nr {
        bail!("POLVAL: NP ({}) must be >= NR ({})", np, nr);
    }
    if nr > p_array.len() {
        bail!(
            "POLVAL: polynomial array too short at index {}",
            p_array.len()
        );
    }

    let mut out = r_array.load_re_vec();
    while out.len() < nr {
        out.push(0.0);
    }

    let cols: Vec<Vec<f64>> = (0..na)
        .map(|v| vec![if v < a_array.len() { a_array[v] } else { 0.0 }])
        .collect();
    let mut tmp = vec![vec![0.0]; nr];
    eval_polys_at_points(&p_array[..nr], &cols, na, 1, &mut tmp)?;
    for i in 0..nr {
        out[i] = tmp[i][0];
    }
    r_array.store_re_vec(out);

    Ok(())
}

/// Batch-evaluate NP polynomials at multiple particles simultaneously.
///
/// Each element of `a_array` is a VE (`Vec<f64>`) of particle values for one
/// coordinate axis. For example, `a_array\[0\]` holds x-values for all particles,
/// `a_array\[1\]` holds px-values, etc.
///
/// Results are written the same way: `r_array\[i\]` will hold the i-th result
/// component for all particles.
///
/// With `nightly-simd`: processes 4 particles per Horner node using f64x4 SIMD.
pub fn rosy_polval_ve(
    l: f64,
    p_array: &[DA],
    np: usize,
    a_array: &[Vec<f64>],
    na: usize,
    r_array: &mut Vec<Vec<f64>>,
    nr: usize,
) -> Result<()> {
    if polval_is_tree_prep(l) {
        return Ok(());
    }
    if np < nr {
        bail!("POLVAL: NP ({}) must be >= NR ({})", np, nr);
    }
    if a_array.len() < na {
        bail!(
            "POLVAL: argument array has {} elements but NA={}",
            a_array.len(),
            na
        );
    }
    if nr > p_array.len() {
        bail!(
            "POLVAL: polynomial array too short at index {}",
            p_array.len()
        );
    }

    let mut num_particles = if na > 0 { a_array[0].len() } else { 0 };
    for v in 1..na {
        num_particles = num_particles.min(a_array[v].len());
    }

    while r_array.len() < nr {
        r_array.push(Vec::new());
    }
    for i in 0..nr {
        r_array[i].resize(num_particles, 0.0);
    }

    eval_polys_at_points(&p_array[..nr], a_array, na, num_particles, r_array)?;
    Ok(())
}

/// Substitute NA Taylor-series arguments into NP Taylor-series polynomials,
/// producing NR Taylor-series results — the COSY map-composition path.
///
/// Each `p_array[i]` is a polynomial in canonical DA variables (e.g. δx, δp_x …);
/// each `a_array[j]` is itself a Taylor series in the *same* canonical variables
/// (typically a saved map's j-th component). The output `r_array[i]` is the
/// composition `p_array[i] ∘ a_array`, truncated automatically to the current
/// truncation order via DA's overloaded `*` and `+`.
///
/// Intermediate monomial values are computed once and reused across all NR
/// polynomials (and across terms of each polynomial).
///
/// COSY's `ANM N M O` lowers to `POLVAL 1 N TWOND MM NV O TWOND` where MM is N's
/// map padded with identity DAs for non-physical slots — see libcosy/physics/map_ops.rosy.
pub fn rosy_polval_da(
    l: f64,
    p_array: &[DA],
    np: usize,
    a_array: &[DA],
    na: usize,
    r_array: &mut Vec<DA>,
    nr: usize,
) -> Result<()> {
    if polval_is_tree_prep(l) {
        return Ok(());
    }
    if np < nr {
        bail!("POLVAL: NP ({}) must be >= NR ({})", np, nr);
    }
    if a_array.len() < na {
        bail!(
            "POLVAL: argument array has {} elements but NA={}",
            a_array.len(),
            na
        );
    }
    if nr > p_array.len() {
        bail!(
            "POLVAL: polynomial array too short at index {}",
            p_array.len()
        );
    }

    while r_array.len() < nr {
        r_array.push(DA::zero());
    }

    if let Some(values) = cached_arg_powers(&p_array[..nr], a_array, na)? {
        for i in 0..nr {
            r_array[i] = dot_poly_da(&p_array[i], &values)?;
        }
        return Ok(());
    }
    let values = compose_monomial_values_da(&p_array[..nr], a_array, na)?;
    for i in 0..nr {
        r_array[i] = dot_poly_da(&p_array[i], &values)?;
    }

    Ok(())
}

/// CPOLVAL — complex-DA polynomial composition. Companion to `rosy_polval_da`
/// where every coefficient algebra is over `Complex64` instead of `f64`.
///
/// DANF / NF / TS / TP and the COSY-to-circular conversion procedures (COCR /
/// CRCO) all chain through this — once a normal-form transformation is
/// constructed it lives in CD-space, and the resonance / tune / Twiss
/// extraction relies on substituting CD bases into CD polynomials.
///
/// Algorithmically identical to `rosy_polval_da` — only the coefficient
/// type differs. The DA struct is generic over `T: DACoefficient` and
/// `Complex64` implements that trait, so the same `*` / `+` / `clone()`
/// API works through the type alias.
pub fn rosy_polval_cd(
    l: impl crate::IntoF64,
    p_array: &impl crate::AsCdRef,
    np: impl crate::IntoF64,
    a_array: &impl crate::AsCdRef,
    na: impl crate::IntoF64,
    r_array: &mut impl crate::AsCdDst,
    nr: impl crate::IntoF64,
) -> Result<()> {
    if polval_is_tree_prep(l.into_f64()) {
        return Ok(());
    }
    let p_array = p_array.as_cd_vec();
    let a_array = a_array.as_cd_vec();
    let np = crate::rosy_as_usize(&np.into_f64());
    let na = crate::rosy_as_usize(&na.into_f64());
    let nr = crate::rosy_as_usize(&nr.into_f64());
    if np < nr {
        bail!("CPOLVAL: NP ({}) must be >= NR ({})", np, nr);
    }
    if a_array.len() < na {
        bail!(
            "CPOLVAL: argument array has {} elements but NA={}",
            a_array.len(),
            na
        );
    }

    let mut out = r_array.load_cd_vec();
    while out.len() < nr {
        out.push(CD::zero());
    }

    if nr > p_array.len() {
        bail!(
            "CPOLVAL: polynomial array too short at index {}",
            p_array.len()
        );
    }

    let values = compose_monomial_values_cd(&p_array[..nr], &a_array, na)?;
    for i in 0..nr {
        out[i] = dot_poly_cd(&p_array[i], &values)?;
    }
    r_array.store_cd_vec(out);

    Ok(())
}

// ============================================================================
// Horner factorization — shared by RE, VE, DA, and CD paths
// ============================================================================

/// Compact Horner tree for the union of nonzero monomials in `polys`.
///
/// Node 0 is the constant 1. Node `k>0` is node `parent[k]` times `x_{var[k]}`.
struct HornerPlan {
    /// compact id → original monomial index
    ids: Vec<u32>,
    /// compact id → parent compact id
    parent: Vec<u32>,
    /// compact id → variable (0-based) to multiply by
    var: Vec<u8>,
    /// original monomial index → compact id (`u32::MAX` if unused)
    compact: Vec<u32>,
}

fn horner_plan(polys: &[DA], na: usize) -> Result<HornerPlan> {
    horner_plan_from_nonzero(polys.iter().map(|p| p.nonzero.as_slice()), na)
}

fn horner_plan_from_nonzero<'a>(
    nonzero_lists: impl Iterator<Item = &'a [u32]>,
    na: usize,
) -> Result<HornerPlan> {
    let rt = crate::taylor::get_runtime()?;
    let n = rt.num_monomials;
    let mut needed = vec![false; n];
    needed[0] = true;
    for nz in nonzero_lists {
        for &i in nz {
            let mut k = i as usize;
            if k >= n {
                continue;
            }
            while k != 0 && !needed[k] {
                let v = rt.horner_var[k] as usize;
                if v >= na {
                    bail!("POLVAL: variable index {} out of range (NA={})", v + 1, na);
                }
                needed[k] = true;
                k = rt.horner_parent[k] as usize;
            }
        }
    }

    let mut compact = vec![u32::MAX; n];
    let mut ids = Vec::new();
    compact[0] = 0;
    ids.push(0);
    for k in 1..n {
        if needed[k] {
            compact[k] = ids.len() as u32;
            ids.push(k as u32);
        }
    }

    let mut parent = vec![0u32; ids.len()];
    let mut var = vec![0u8; ids.len()];
    for (c, &k) in ids.iter().enumerate().skip(1) {
        let k = k as usize;
        parent[c] = compact[rt.horner_parent[k] as usize];
        var[c] = rt.horner_var[k];
    }

    Ok(HornerPlan {
        ids,
        parent,
        var,
        compact,
    })
}

fn eval_polys_at_points(
    polys: &[DA],
    a_array: &[Vec<f64>],
    na: usize,
    npart: usize,
    outs: &mut [Vec<f64>],
) -> Result<()> {
    if polys.is_empty() {
        return Ok(());
    }
    let plan = horner_plan(polys, na)?;
    let n_nodes = plan.ids.len();

    if npart == 0 {
        for out in outs.iter_mut().take(polys.len()) {
            out.clear();
        }
        return Ok(());
    }

    let mut mval = vec![0.0; n_nodes * npart];
    for p in 0..npart {
        mval[p] = 1.0;
    }

    for k in 1..n_nodes {
        let v = plan.var[k] as usize;
        let xs = &a_array[v];
        let o = k * npart;
        let po = plan.parent[k] as usize * npart;

        #[cfg(feature = "nightly-simd")]
        {
            let chunks = npart / LANES;
            for c in 0..chunks {
                let base = c * LANES;
                let pv = Simd::<f64, LANES>::from_slice(&mval[po + base..]);
                let xv = Simd::<f64, LANES>::from_slice(&xs[base..]);
                (pv * xv).copy_to_slice(&mut mval[o + base..o + base + LANES]);
            }
            for p in (chunks * LANES)..npart {
                mval[o + p] = mval[po + p] * xs[p];
            }
        }

        #[cfg(not(feature = "nightly-simd"))]
        {
            for p in 0..npart {
                mval[o + p] = mval[po + p] * xs[p];
            }
        }
    }

    for (i, poly) in polys.iter().enumerate() {
        let out = &mut outs[i];
        out.fill(0.0);
        for &idx in &poly.nonzero {
            let c = poly.coeffs[idx as usize];
            if c == 0.0 {
                continue;
            }
            let mk = plan.compact[idx as usize] as usize * npart;

            #[cfg(feature = "nightly-simd")]
            {
                let chunks = npart / LANES;
                let cv = Simd::<f64, LANES>::splat(c);
                for ch in 0..chunks {
                    let base = ch * LANES;
                    let acc = Simd::<f64, LANES>::from_slice(&out[base..]);
                    let mv = Simd::<f64, LANES>::from_slice(&mval[mk + base..]);
                    mv.mul_add(cv, acc)
                        .copy_to_slice(&mut out[base..base + LANES]);
                }
                for p in (chunks * LANES)..npart {
                    out[p] = crate::taylor::da::fmadd(c, mval[mk + p], out[p]);
                }
            }

            #[cfg(not(feature = "nightly-simd"))]
            {
                for p in 0..npart {
                    out[p] = crate::taylor::da::fmadd(c, mval[mk + p], out[p]);
                }
            }
        }
    }

    Ok(())
}

struct DaMonomialValues {
    compact: Vec<u32>,
    values: Vec<DA>,
}

/// Every power a POLVAL argument set can produce, kept for exact reuse.
///
/// PHAD's M2L composes each expansion with a translation map that depends only
/// on the offset between two boxes, and those maps repeat thousands of times
/// a step. Building the powers is 83 series products per call; the weighted
/// sum after it is cheap. Each power here comes from the same product chain a
/// per-call plan uses (`horner_parent`), so a reused power is bit-identical.
struct ArgPowers {
    args: Vec<DA>,
    max_order: u32,
    eps_bits: u64,
    num_monomials: usize,
    values: Rc<DaMonomialValues>,
    bytes: usize,
    last_use: u64,
}

#[derive(Default)]
struct PowerCache {
    map: HashMap<u64, Vec<ArgPowers>>,
    /// Use order, oldest first: `(key, stamp)` per insert or hit. An entry
    /// whose stamp no longer matches its `last_use` is stale and skipped, so
    /// evicting the least recently used map is O(1) amortized.
    order: std::collections::VecDeque<(u64, u64)>,
    clock: u64,
    bytes: usize,
}

/// Memory one thread's cache may hold, in MB (`ROSY_POLVAL_CACHE_MB`, 0 = off).
/// An order-6, 3-variable entry is about 85 KB, so the default keeps about
/// 1500 maps. In PHAD's FMM 71% of calls repeat an earlier map, nearly all
/// within 2048 calls of it.
fn power_cache_bytes() -> usize {
    use std::sync::OnceLock;
    static LIMIT: OnceLock<usize> = OnceLock::new();
    *LIMIT.get_or_init(|| {
        std::env::var("ROSY_POLVAL_CACHE_MB")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(128)
            << 20
    })
}

thread_local! {
    static POWER_CACHE: RefCell<PowerCache> = RefCell::new(PowerCache::default());
}

fn same_da(a: &DA, b: &DA) -> bool {
    a.nonzero == b.nonzero
        && a.nonzero
            .iter()
            .all(|&i| a.coeffs[i as usize].to_bits() == b.coeffs[i as usize].to_bits())
}

/// Powers for `args`, from the cache or built and kept. `None` = take the
/// per-call path (a polynomial uses a variable past `na`, which that path
/// reports, or the runtime has no monomial table).
fn cached_arg_powers(polys: &[DA], args: &[DA], na: usize) -> Result<Option<Rc<DaMonomialValues>>> {
    use std::hash::{Hash, Hasher};
    if power_cache_bytes() == 0 {
        return Ok(None);
    }
    let rt = crate::taylor::get_runtime()?;
    let n = rt.num_monomials;
    if na == 0 || args.len() < na || args[..na].iter().any(|a| a.coeffs.len() != n) {
        return Ok(None);
    }
    let max_order = rt.config.max_order;
    let eps_bits = rt.config.epsilon.to_bits();
    let mut h = rustc_hash::FxHasher::default();
    (max_order, eps_bits, n, na).hash(&mut h);
    for a in &args[..na] {
        a.nonzero.hash(&mut h);
        for &i in &a.nonzero {
            a.coeffs[i as usize].to_bits().hash(&mut h);
        }
    }
    let key = h.finish();

    let found = POWER_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        c.clock += 1;
        let now = c.clock;
        let hit = c.map.get_mut(&key).and_then(|bucket| {
            bucket
                .iter_mut()
                .find(|e| {
                    e.max_order == max_order
                        && e.eps_bits == eps_bits
                        && e.num_monomials == n
                        && e.args.len() == na
                        && e.args.iter().zip(&args[..na]).all(|(x, y)| same_da(x, y))
                })
                .map(|e| {
                    e.last_use = now;
                    e.values.clone()
                })
        });
        if hit.is_some() {
            c.order.push_back((key, now));
        }
        hit
    });
    let values = match found {
        Some(v) => v,
        None => {
            // Every monomial in the first `na` variables, in index order.
            let usable: Vec<u32> = (0..n as u32)
                .filter(|&k| {
                    rt.monomial_list[k as usize].exponents[na.min(crate::taylor::MAX_VARS)..]
                        .iter()
                        .all(|&e| e == 0)
                })
                .collect();
            let plan = horner_plan_from_nonzero(std::iter::once(usable.as_slice()), na)?;
            let mut values = Vec::with_capacity(plan.ids.len());
            values.push(DA::from_coeff(1.0));
            for k in 1..plan.ids.len() {
                let parent = &values[plan.parent[k] as usize];
                let arg = &args[plan.var[k] as usize];
                values.push((parent * arg)?);
            }
            let bytes = values
                .iter()
                .map(|v| v.coeffs.len() * 8 + v.nonzero.capacity() * 4)
                .sum::<usize>()
                + plan.compact.len() * 4;
            let values = Rc::new(DaMonomialValues {
                compact: plan.compact,
                values,
            });
            POWER_CACHE.with(|c| {
                let mut c = c.borrow_mut();
                let limit = power_cache_bytes();
                if bytes > limit {
                    return;
                }
                while c.bytes + bytes > limit {
                    // Drop the least recently used map; skip stale order entries.
                    let Some((k, stamp)) = c.order.pop_front() else {
                        break;
                    };
                    let mut freed = 0;
                    if let Some(bucket) = c.map.get_mut(&k) {
                        if let Some(pos) = bucket.iter().position(|e| e.last_use == stamp) {
                            freed = bucket.remove(pos).bytes;
                        }
                        if bucket.is_empty() {
                            c.map.remove(&k);
                        }
                    }
                    c.bytes = c.bytes.saturating_sub(freed);
                }
                // Hits leave stale order entries behind; keep the queue bounded.
                let live: usize = c.map.values().map(|b| b.len()).sum();
                if c.order.len() > 8 * live.max(64) {
                    let mut stamps: Vec<(u64, u64)> = c
                        .map
                        .iter()
                        .flat_map(|(k, b)| b.iter().map(move |e| (e.last_use, *k)))
                        .collect();
                    stamps.sort_unstable();
                    c.order = stamps.into_iter().map(|(s, k)| (k, s)).collect();
                }
                let now = c.clock;
                c.bytes += bytes;
                c.order.push_back((key, now));
                c.map.entry(key).or_default().push(ArgPowers {
                    args: args[..na].to_vec(),
                    max_order,
                    eps_bits,
                    num_monomials: n,
                    values: values.clone(),
                    bytes,
                    last_use: now,
                });
            });
            values
        }
    };
    // A polynomial term outside the first `na` variables: the per-call path
    // reports it.
    let covered = polys.iter().all(|p| {
        p.nonzero.iter().all(|&i| {
            values
                .compact
                .get(i as usize)
                .is_some_and(|&c| c != u32::MAX)
        })
    });
    Ok(covered.then_some(values))
}

fn compose_monomial_values_da(polys: &[DA], args: &[DA], na: usize) -> Result<DaMonomialValues> {
    let plan = horner_plan(polys, na)?;
    let mut values = Vec::with_capacity(plan.ids.len());
    values.push(DA::from_coeff(1.0));
    for k in 1..plan.ids.len() {
        let parent = &values[plan.parent[k] as usize];
        let arg = &args[plan.var[k] as usize];
        values.push((parent * arg)?);
    }
    Ok(DaMonomialValues {
        compact: plan.compact,
        values,
    })
}

/// `sum c_i * value_i`, accumulated in one buffer. Same rounding, cutoff,
/// and term order as `result = result + value_i * c_i` term by term, without
/// two temporaries per monomial (M2L composes once per interaction pair).
fn dot_poly_da(poly: &DA, values: &DaMonomialValues) -> Result<DA> {
    let rt = crate::taylor::get_runtime()?;
    let eps = rt.config.epsilon;
    let max_order = rt.config.max_order as u8;
    let orders = &rt.monomial_orders;
    let mut result = DA::from_coeff(poly.coeffs.first().copied().unwrap_or(0.0));
    let mut present = vec![false; result.coeffs.len()];
    for &k in &result.nonzero {
        present[k as usize] = true;
    }
    let mut dropped = false;
    for &idx in &poly.nonzero {
        if idx == 0 {
            continue;
        }
        let c = poly.coeffs[idx as usize];
        if c.abs() <= eps {
            continue;
        }
        let node = values.compact[idx as usize] as usize;
        let term = &values.values[node];
        for &k in &term.nonzero {
            let ku = k as usize;
            let t = term.coeffs[ku] * c;
            if t.abs() <= eps || orders[ku] > max_order {
                continue;
            }
            let sum = result.coeffs[ku] + t;
            if sum.abs() > eps {
                result.coeffs[ku] = sum;
                if !present[ku] {
                    present[ku] = true;
                    result.nonzero.push(k);
                }
            } else {
                result.coeffs[ku] = 0.0;
                if present[ku] {
                    present[ku] = false;
                    dropped = true;
                }
            }
        }
        if dropped {
            result.nonzero.retain(|&k| present[k as usize]);
            dropped = false;
        }
    }
    Ok(result)
}

struct CdMonomialValues {
    compact: Vec<u32>,
    values: Vec<CD>,
}

fn compose_monomial_values_cd(polys: &[CD], args: &[CD], na: usize) -> Result<CdMonomialValues> {
    let plan = horner_plan_from_nonzero(polys.iter().map(|p| p.nonzero.as_slice()), na)?;
    let mut values = Vec::with_capacity(plan.ids.len());
    values.push(CD::from_coeff(num_complex::Complex64::new(1.0, 0.0)));
    for k in 1..plan.ids.len() {
        let parent = &values[plan.parent[k] as usize];
        let arg = &args[plan.var[k] as usize];
        values.push((parent * arg)?);
    }
    Ok(CdMonomialValues {
        compact: plan.compact,
        values,
    })
}

fn dot_poly_cd(poly: &CD, values: &CdMonomialValues) -> Result<CD> {
    let c0 = poly
        .coeffs
        .first()
        .copied()
        .unwrap_or(num_complex::Complex64::new(0.0, 0.0));
    let mut result = CD::from_coeff(c0);
    for &idx in &poly.nonzero {
        if idx == 0 {
            continue;
        }
        let c = poly.coeffs[idx as usize];
        let node = values.compact[idx as usize] as usize;
        result = (&result + &(&values.values[node] * c)?)?;
    }
    Ok(result)
}

/// CD-typed exponentiation by repeated squaring. Mirrors `da_powi`.
#[inline]
pub fn cd_powi(base: &CD, exp: u8) -> Result<CD> {
    Ok(match exp {
        0 => CD::from_coeff(num_complex::Complex64::new(1.0, 0.0)),
        1 => base.clone(),
        2 => (base.clone() * base)?,
        3 => {
            let v2 = (base.clone() * base)?;
            (v2 * base)?
        }
        4 => {
            let v2 = (base.clone() * base)?;
            (v2.clone() * &v2)?
        }
        5 => {
            let v2 = (base.clone() * base)?;
            let v4 = (v2.clone() * &v2)?;
            (v4 * base)?
        }
        6 => {
            let v2 = (base.clone() * base)?;
            let v3 = (v2 * base)?;
            (v3.clone() * &v3)?
        }
        _ => {
            let mut result = CD::from_coeff(num_complex::Complex64::new(1.0, 0.0));
            let mut current = base.clone();
            let mut e = exp;
            while e > 0 {
                if e & 1 == 1 {
                    result = (result * &current)?;
                }
                current = (current.clone() * &current)?;
                e >>= 1;
            }
            result
        }
    })
}

/// Compute base^exp for small u8 exponents using exponentiation-by-squaring.
#[inline]
pub fn da_powi(base: &DA, exp: u8) -> Result<DA> {
    Ok(match exp {
        0 => DA::from_coeff(1.0),
        1 => base.clone(),
        2 => (base.clone() * base)?,
        3 => {
            let v2 = (base.clone() * base)?;
            (v2 * base)?
        }
        4 => {
            let v2 = (base.clone() * base)?;
            (v2.clone() * &v2)?
        }
        5 => {
            let v2 = (base.clone() * base)?;
            let v4 = (v2.clone() * &v2)?;
            (v4 * base)?
        }
        6 => {
            let v2 = (base.clone() * base)?;
            let v3 = (v2 * base)?;
            (v3.clone() * &v3)?
        }
        _ => {
            let mut result = DA::from_coeff(1.0);
            let mut current = base.clone();
            let mut e = exp;
            while e > 0 {
                if e & 1 == 1 {
                    result = (result * &current)?;
                }
                current = (current.clone() * &current)?;
                e >>= 1;
            }
            result
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RosyValue;

    #[serial_test::serial]
    #[test]
    fn polval_any_composes_da_map_on_cd_args() -> anyhow::Result<()> {
        crate::taylor::cleanup_taylor();
        crate::taylor::init_taylor(2, 2)?;
        let p = vec![RosyValue::DA(DA::variable(1)?)];
        let a = vec![
            RosyValue::CD(CD::from_da(&DA::variable(2)?)),
            RosyValue::CD(CD::from_da(&DA::variable(1)?)),
        ];
        let mut out = RosyValue::RE(0.0);
        rosy_polval_any(1f64, &p, 1f64, &a, 2f64, &mut out, 1f64)?;
        let RosyValue::CD(cd) = out else {
            panic!("expected CD");
        };
        // P = x1, A = (x2, x1) as CD → result is the CD for x2
        let mut x2_coeff = 0.0;
        for (mono, c) in cd.real_part().coeffs_iter() {
            if mono.exponents.get(1).copied() == Some(1) && mono.total_order == 1 {
                x2_coeff = c;
            }
        }
        assert!((x2_coeff - 1.0).abs() < 1e-12, "x2 coeff {x2_coeff}");
        crate::taylor::cleanup_taylor();
        Ok(())
    }

    #[serial_test::serial]
    #[test]
    fn polval_re_quadratic() -> anyhow::Result<()> {
        crate::taylor::cleanup_taylor();
        crate::taylor::init_taylor(3, 1)?;
        let x = DA::variable(1)?;
        let p = (&DA::from_coeff(2.0) + &(&x * 3.0)?)?;
        let p = (&p + &(&x * &x)?)?;
        let mut r = vec![0.0];
        rosy_polval_re(1.0, &vec![p], 1.0, &vec![2.0], 1.0, &mut r, 1.0)?;
        assert!((r[0] - 12.0).abs() < 1e-12, "got {}", r[0]);
        crate::taylor::cleanup_taylor();
        Ok(())
    }

    #[serial_test::serial]
    #[test]
    fn polval_ve_matches_re_and_closed_form() -> anyhow::Result<()> {
        crate::taylor::cleanup_taylor();
        crate::taylor::init_taylor(4, 2)?;
        let x = DA::variable(1)?;
        let y = DA::variable(2)?;
        let p = (&DA::from_coeff(3.0) + &((&(&x * &x)? * &y)?))?;
        let xs = vec![1.0, 2.0, -1.0];
        let ys = vec![4.0, 5.0, 6.0];
        let mut out = vec![vec![]];
        rosy_polval_ve(
            1.0,
            &[p.clone()],
            1,
            &[xs.clone(), ys.clone()],
            2,
            &mut out,
            1,
        )?;
        for i in 0..3 {
            let mut r = vec![0.0];
            rosy_polval_re(
                1.0,
                &vec![p.clone()],
                1.0,
                &vec![xs[i], ys[i]],
                2.0,
                &mut r,
                1.0,
            )?;
            let expect = xs[i] * xs[i] * ys[i] + 3.0;
            assert!(
                (out[0][i] - r[0]).abs() < 1e-12,
                "ve {} vs re {}",
                out[0][i],
                r[0]
            );
            assert!((r[0] - expect).abs() < 1e-12, "re {} vs {expect}", r[0]);
        }
        crate::taylor::cleanup_taylor();
        Ok(())
    }

    #[serial_test::serial]
    #[test]
    fn polval_da_compose_square() -> anyhow::Result<()> {
        crate::taylor::cleanup_taylor();
        crate::taylor::init_taylor(3, 2)?;
        let x = DA::variable(1)?;
        let y = DA::variable(2)?;
        let p = (&x * &x)?;
        let a1 = (&DA::from_coeff(1.0) + &y)?;
        let mut r = Vec::new();
        rosy_polval_da(1.0, &[p], 1, &[a1, DA::zero()], 2, &mut r, 1)?;
        // (1+y)^2 = 1 + 2y + y^2
        assert!((r[0].constant_part() - 1.0).abs() < 1e-12);
        let y_idx = crate::taylor::get_runtime()?.variable_indices[1] as usize;
        assert!((r[0].coeffs[y_idx] - 2.0).abs() < 1e-12, "2y coeff");
        crate::taylor::cleanup_taylor();
        Ok(())
    }

    #[serial_test::serial]
    #[test]
    fn polval_two_polys_share_horner_nodes() -> anyhow::Result<()> {
        crate::taylor::cleanup_taylor();
        crate::taylor::init_taylor(3, 2)?;
        let x = DA::variable(1)?;
        let y = DA::variable(2)?;
        let p1 = (&x * &y)?;
        let p2 = (&x * &x)?;
        let mut r = vec![0.0, 0.0];
        rosy_polval_re(1.0, &vec![p1, p2], 2.0, &vec![3.0, 4.0], 2.0, &mut r, 2.0)?;
        assert!((r[0] - 12.0).abs() < 1e-12);
        assert!((r[1] - 9.0).abs() < 1e-12);
        crate::taylor::cleanup_taylor();
        Ok(())
    }

    #[serial_test::serial]
    #[test]
    fn polval_105_does_not_evaluate() -> anyhow::Result<()> {
        crate::taylor::cleanup_taylor();
        crate::taylor::init_taylor(2, 1)?;
        let x = DA::variable(1)?;
        let p = vec![RosyValue::DA((&DA::from_coeff(1.0) + &x)?)];
        let a = vec![RosyValue::VE(vec![0.0, 3.0])];
        let mut out = vec![RosyValue::VE(vec![99.0, 99.0])];
        rosy_polval_any(105f64, &p, 1f64, &a, 1f64, &mut out, 1f64)?;
        match &out[0] {
            RosyValue::VE(xs) => assert_eq!(xs, &vec![99.0, 99.0]),
            other => panic!("prep must not rewrite output, got {}", other.kind_name()),
        }
        rosy_polval_any(-5f64, &p, 1f64, &a, 1f64, &mut out, 1f64)?;
        match &out[0] {
            RosyValue::Arr(xs) => {
                assert!((xs[0].as_f64() - 1.0).abs() < 1e-12);
                assert!((xs[1].as_f64() - 4.0).abs() < 1e-12);
            }
            RosyValue::VE(xs) => {
                assert!((xs[0] - 1.0).abs() < 1e-12);
                assert!((xs[1] - 4.0).abs() < 1e-12);
            }
            other => panic!("eval should write particles, got {}", other.kind_name()),
        }
        crate::taylor::cleanup_taylor();
        Ok(())
    }
}
