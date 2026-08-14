//! Memory serialization runtime support for Rosy.
//!
//! Implements WRITEM/READM semantics: serialize a Rosy variable into parallel
//! arrays (double-precision part, integer metadata part, DA parameters) and
//! deserialize back again.
//!
//! ## Rosy serialization format
//!
//! WRITEM fills four outputs:
//!
//! - `var_info: Vec\<f64\>` — `[type_code, payload_length, version_id]`
//! - `dp_array: Vec\<f64\>` — double-precision payload
//! - `int_array: Vec\<f64\>` — integer metadata (stored as f64)
//! - `da_params: Vec\<f64\>` — DA parameters `[order, nv]`, or `[0.0]` for non-DA
//!
//! Type codes (mirroring COSY conventions):
//! - 1 = RE (f64)
//! - 2 = ST (String)
//! - 3 = LO (bool)
//! - 4 = CM (Complex64)
//! - 5 = VE (Vec\<f64\>)
//! - 6 = DA
//! - 7 = CD
//!
//! Version ID is always 1 for this format.

use anyhow::{Result, bail};

/// Version identifier embedded in WRITEM output.
pub const WRITEM_VERSION: f64 = 1.0;

// ──────────────────────────────────────────────────────────────────────────────
// Trait definitions
// ──────────────────────────────────────────────────────────────────────────────

/// Serialize a Rosy variable to the WRITEM array representation.
///
/// Returns `(var_info, dp_array, int_array, da_params)`.
pub trait RosyWritem {
    fn writem(&self) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>);
}

/// Deserialize a Rosy variable from the WRITEM array representation.
pub trait RosyReadm: Sized {
    /// Expected type code for this type (used for validation).
    fn expected_type_code() -> f64;

    fn readm(
        var_info: &[f64],
        length: f64,
        dp_array: &[f64],
        int_array: &[f64],
        da_params: &[f64],
    ) -> Result<Self>;
}

// ──────────────────────────────────────────────────────────────────────────────
// RE (f64)
// ──────────────────────────────────────────────────────────────────────────────

impl RosyWritem for f64 {
    fn writem(&self) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
        let dp = vec![*self];
        let int = vec![];
        let da_params = vec![0.0];
        let var_info = vec![1.0, dp.len() as f64, WRITEM_VERSION];
        (var_info, dp, int, da_params)
    }
}

impl RosyReadm for f64 {
    fn expected_type_code() -> f64 {
        1.0
    }

    fn readm(
        var_info: &[f64],
        _length: f64,
        dp_array: &[f64],
        _int_array: &[f64],
        _da_params: &[f64],
    ) -> Result<Self> {
        // Fix #1: bounds check before var_info[1] access
        if var_info.len() < 2 {
            bail!("READM: var_info has fewer than 2 elements (too short to read type/length)");
        }
        // Fix #2: type code validation
        let type_code = var_info[0];
        if (type_code - Self::expected_type_code()).abs() > 0.5 {
            bail!(
                "READM: type code mismatch — buffer contains type {type_code} but expected {} (RE)",
                Self::expected_type_code()
            );
        }
        if dp_array.is_empty() {
            bail!("READM: dp_array is empty for RE type");
        }
        Ok(dp_array[0])
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// ST (String)
// ──────────────────────────────────────────────────────────────────────────────

impl RosyWritem for String {
    fn writem(&self) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
        // Encode each byte of the string as one f64 in int_array.
        let int: Vec<f64> = self.as_bytes().iter().map(|&b| b as f64).collect();
        let dp = vec![];
        let da_params = vec![0.0];
        let var_info = vec![2.0, int.len() as f64, WRITEM_VERSION];
        (var_info, dp, int, da_params)
    }
}

impl RosyReadm for String {
    fn expected_type_code() -> f64 {
        2.0
    }

    fn readm(
        var_info: &[f64],
        _length: f64,
        _dp_array: &[f64],
        int_array: &[f64],
        _da_params: &[f64],
    ) -> Result<Self> {
        if var_info.len() < 2 {
            bail!("READM: var_info has fewer than 2 elements (too short to read type/length)");
        }
        let type_code = var_info[0];
        if (type_code - Self::expected_type_code()).abs() > 0.5 {
            bail!(
                "READM: type code mismatch — buffer contains type {type_code} but expected {} (ST)",
                Self::expected_type_code()
            );
        }
        let bytes: Vec<u8> = int_array.iter().map(|&v| v as u8).collect();
        String::from_utf8(bytes)
            .map_err(|e| anyhow::anyhow!("READM: invalid UTF-8 in ST reconstruction: {}", e))
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// LO (bool)
// ──────────────────────────────────────────────────────────────────────────────

impl RosyWritem for bool {
    fn writem(&self) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
        let int = vec![if *self { 1.0 } else { 0.0 }];
        let dp = vec![];
        let da_params = vec![0.0];
        let var_info = vec![3.0, int.len() as f64, WRITEM_VERSION];
        (var_info, dp, int, da_params)
    }
}

impl RosyReadm for bool {
    fn expected_type_code() -> f64 {
        3.0
    }

    fn readm(
        var_info: &[f64],
        _length: f64,
        _dp_array: &[f64],
        int_array: &[f64],
        _da_params: &[f64],
    ) -> Result<Self> {
        if var_info.len() < 2 {
            bail!("READM: var_info has fewer than 2 elements (too short to read type/length)");
        }
        let type_code = var_info[0];
        if (type_code - Self::expected_type_code()).abs() > 0.5 {
            bail!(
                "READM: type code mismatch — buffer contains type {type_code} but expected {} (LO)",
                Self::expected_type_code()
            );
        }
        if int_array.is_empty() {
            bail!("READM: int_array is empty for LO type");
        }
        Ok(int_array[0] != 0.0)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// CM (Complex64)
// ──────────────────────────────────────────────────────────────────────────────

impl RosyWritem for num_complex::Complex64 {
    fn writem(&self) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
        let dp = vec![self.re, self.im];
        let int = vec![];
        let da_params = vec![0.0];
        let var_info = vec![4.0, dp.len() as f64, WRITEM_VERSION];
        (var_info, dp, int, da_params)
    }
}

impl RosyReadm for num_complex::Complex64 {
    fn expected_type_code() -> f64 {
        4.0
    }

    fn readm(
        var_info: &[f64],
        _length: f64,
        dp_array: &[f64],
        _int_array: &[f64],
        _da_params: &[f64],
    ) -> Result<Self> {
        if var_info.len() < 2 {
            bail!("READM: var_info has fewer than 2 elements (too short to read type/length)");
        }
        let type_code = var_info[0];
        if (type_code - Self::expected_type_code()).abs() > 0.5 {
            bail!(
                "READM: type code mismatch — buffer contains type {type_code} but expected {} (CM)",
                Self::expected_type_code()
            );
        }
        if dp_array.len() < 2 {
            bail!("READM: dp_array has fewer than 2 elements for CM type");
        }
        Ok(num_complex::Complex64::new(dp_array[0], dp_array[1]))
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// VE (Vec<f64>)
// ──────────────────────────────────────────────────────────────────────────────

impl RosyWritem for Vec<f64> {
    fn writem(&self) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
        let dp = self.clone();
        let int = vec![];
        let da_params = vec![0.0];
        let var_info = vec![5.0, dp.len() as f64, WRITEM_VERSION];
        (var_info, dp, int, da_params)
    }
}

impl RosyReadm for Vec<f64> {
    fn expected_type_code() -> f64 {
        5.0
    }

    fn readm(
        var_info: &[f64],
        _length: f64,
        dp_array: &[f64],
        _int_array: &[f64],
        _da_params: &[f64],
    ) -> Result<Self> {
        // Fix #1: bounds check before var_info[1] access
        if var_info.len() < 2 {
            bail!("READM: var_info has fewer than 2 elements (too short to read type/length)");
        }
        // Fix #2: type code validation
        let type_code = var_info[0];
        if (type_code - Self::expected_type_code()).abs() > 0.5 {
            bail!(
                "READM: type code mismatch — buffer contains type {type_code} but expected {} (VE)",
                Self::expected_type_code()
            );
        }
        // Use the stored payload_length from var_info[1] (authoritative)
        let n = var_info[1] as usize;
        if dp_array.len() < n {
            bail!(
                "READM: dp_array has {} elements but VE length is {}",
                dp_array.len(),
                n
            );
        }
        Ok(dp_array[..n].to_vec())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// DA (DA<f64>)
//
// Serialization format:
//   dp_array  = [coeff_0, coeff_1, ...]    (one entry per nonzero term)
//   int_array = [exp_0_0, exp_0_1, ..., exp_0_{MAX_VARS-1},
//                exp_1_0, ..., ...]         (MAX_VARS entries per term)
//   da_params = [max_order, num_vars]
// ──────────────────────────────────────────────────────────────────────────────

impl RosyWritem for crate::taylor::DA {
    fn writem(&self) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
        use crate::taylor::{MAX_VARS, get_config};

        let rt_cfg = get_config().expect("Taylor system not initialized");
        let order = rt_cfg.max_order as f64;
        let nv = rt_cfg.num_vars as f64;

        let terms = self.coeffs_iter();
        let mut dp: Vec<f64> = Vec::with_capacity(terms.len());
        let mut int: Vec<f64> = Vec::with_capacity(terms.len() * MAX_VARS);

        for (monomial, coeff) in &terms {
            dp.push(*coeff);
            for i in 0..MAX_VARS {
                int.push(monomial.exponents[i] as f64);
            }
        }

        let da_params = vec![order, nv];
        let var_info = vec![6.0, dp.len() as f64, WRITEM_VERSION];
        (var_info, dp, int, da_params)
    }
}

impl RosyReadm for crate::taylor::DA {
    fn expected_type_code() -> f64 {
        6.0
    }

    fn readm(
        var_info: &[f64],
        _length: f64,
        dp_array: &[f64],
        int_array: &[f64],
        da_params: &[f64],
    ) -> Result<Self> {
        use crate::taylor::{DA, MAX_VARS, Monomial, get_config};
        use rustc_hash::FxHashMap;

        // Fix #1: bounds check before var_info[1] access
        if var_info.len() < 2 {
            bail!("READM: var_info has fewer than 2 elements (too short to read type/length)");
        }
        // Fix #2: type code validation
        let type_code = var_info[0];
        if (type_code - Self::expected_type_code()).abs() > 0.5 {
            bail!(
                "READM: type code mismatch — buffer contains type {type_code} but expected {} (DA)",
                Self::expected_type_code()
            );
        }

        // Fix #3: DA config compatibility check
        if da_params.len() >= 2 {
            let buf_order = da_params[0] as u32;
            let buf_nv = da_params[1] as usize;
            let rt_cfg = get_config()
                .map_err(|e| anyhow::anyhow!("READM: Taylor system not initialized: {}", e))?;
            if buf_order != rt_cfg.max_order || buf_nv != rt_cfg.num_vars {
                bail!(
                    "READM: DA config mismatch — buffer has order={buf_order}, nv={buf_nv} \
                     but current DAINI has order={}, nv={}",
                    rt_cfg.max_order,
                    rt_cfg.num_vars
                );
            }
        }

        let n_terms = var_info[1] as usize;
        let expected_int_len = n_terms * MAX_VARS;

        if dp_array.len() < n_terms {
            bail!(
                "READM: dp_array has {} entries but expected {} DA terms",
                dp_array.len(),
                n_terms
            );
        }
        if int_array.len() < expected_int_len {
            bail!(
                "READM: int_array has {} entries but expected {} ({}*{})",
                int_array.len(),
                expected_int_len,
                n_terms,
                MAX_VARS
            );
        }

        let mut hash_coeffs: FxHashMap<Monomial, f64> = FxHashMap::default();
        for i in 0..n_terms {
            let coeff = dp_array[i];
            let base = i * MAX_VARS;
            let mut exponents = [0u8; MAX_VARS];
            for j in 0..MAX_VARS {
                exponents[j] = int_array[base + j] as u8;
            }
            let mono = Monomial::new(exponents);
            hash_coeffs.insert(mono, coeff);
        }

        Ok(DA::from_coeffs(hash_coeffs))
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// CD (DA<Complex64>) — analogous to DA but stores (re, im) pairs in dp_array
// ──────────────────────────────────────────────────────────────────────────────

impl RosyWritem for crate::taylor::CD {
    fn writem(&self) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
        use crate::taylor::{MAX_VARS, get_config};

        let rt_cfg = get_config().expect("Taylor system not initialized");
        let order = rt_cfg.max_order as f64;
        let nv = rt_cfg.num_vars as f64;

        let terms = self.coeffs_iter();
        let mut dp: Vec<f64> = Vec::with_capacity(terms.len() * 2);
        let mut int: Vec<f64> = Vec::with_capacity(terms.len() * MAX_VARS);

        for (monomial, coeff) in &terms {
            dp.push(coeff.re);
            dp.push(coeff.im);
            for i in 0..MAX_VARS {
                int.push(monomial.exponents[i] as f64);
            }
        }

        // n_terms = dp.len() / 2  — store that as the payload length
        let n_terms = terms.len() as f64;
        let da_params = vec![order, nv];
        let var_info = vec![7.0, n_terms, WRITEM_VERSION];
        (var_info, dp, int, da_params)
    }
}

impl RosyReadm for crate::taylor::CD {
    fn expected_type_code() -> f64 {
        7.0
    }

    fn readm(
        var_info: &[f64],
        _length: f64,
        dp_array: &[f64],
        int_array: &[f64],
        da_params: &[f64],
    ) -> Result<Self> {
        use crate::taylor::{CD, MAX_VARS, Monomial, get_config};
        use num_complex::Complex64;
        use rustc_hash::FxHashMap;

        // Fix #1: bounds check before var_info[1] access
        if var_info.len() < 2 {
            bail!("READM: var_info has fewer than 2 elements (too short to read type/length)");
        }
        // Fix #2: type code validation
        let type_code = var_info[0];
        if (type_code - Self::expected_type_code()).abs() > 0.5 {
            bail!(
                "READM: type code mismatch — buffer contains type {type_code} but expected {} (CD)",
                Self::expected_type_code()
            );
        }

        // Fix #3: CD config compatibility check
        if da_params.len() >= 2 {
            let buf_order = da_params[0] as u32;
            let buf_nv = da_params[1] as usize;
            let rt_cfg = get_config()
                .map_err(|e| anyhow::anyhow!("READM: Taylor system not initialized: {}", e))?;
            if buf_order != rt_cfg.max_order || buf_nv != rt_cfg.num_vars {
                bail!(
                    "READM: CD config mismatch — buffer has order={buf_order}, nv={buf_nv} \
                     but current DAINI has order={}, nv={}",
                    rt_cfg.max_order,
                    rt_cfg.num_vars
                );
            }
        }

        let n_terms = var_info[1] as usize;
        let expected_dp = n_terms * 2;
        let expected_int = n_terms * MAX_VARS;

        if dp_array.len() < expected_dp {
            bail!(
                "READM: dp_array has {} entries but expected {} for CD ({} terms * 2)",
                dp_array.len(),
                expected_dp,
                n_terms
            );
        }
        if int_array.len() < expected_int {
            bail!(
                "READM: int_array has {} entries but expected {} for CD",
                int_array.len(),
                expected_int
            );
        }

        let mut hash_coeffs: FxHashMap<Monomial, Complex64> = FxHashMap::default();
        for i in 0..n_terms {
            let re = dp_array[i * 2];
            let im = dp_array[i * 2 + 1];
            let coeff = Complex64::new(re, im);
            let base = i * MAX_VARS;
            let mut exponents = [0u8; MAX_VARS];
            for j in 0..MAX_VARS {
                exponents[j] = int_array[base + j] as u8;
            }
            let mono = Monomial::new(exponents);
            hash_coeffs.insert(mono, coeff);
        }

        Ok(CD::from_coeffs(hash_coeffs))
    }
}
