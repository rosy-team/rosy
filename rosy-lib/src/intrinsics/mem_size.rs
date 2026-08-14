use crate::{RE, VE};

/// LST(n) - String memory allocation estimator (COSY compatibility).
/// In COSY, LST(n) returns n (the size in 8-byte words for a string of length n).
/// In Rosy, we just pass through the value since Rosy handles memory automatically.
pub trait RosyLST {
    fn rosy_lst(&self) -> RE;
}

impl RosyLST for RE {
    fn rosy_lst(&self) -> RE {
        *self
    }
}

/// LCM(n) - Complex number memory estimator (COSY compatibility).
/// In COSY, LCM(n) always returns 2.0 (a CM always occupies exactly 2 RE words).
/// The input is arbitrary; the result is the constant 2.
pub trait RosyLCM {
    fn rosy_lcm(&self) -> RE;
}

impl RosyLCM for RE {
    fn rosy_lcm(&self) -> RE {
        2.0
    }
}

/// LRE(n) - Real memory size estimator (COSY compatibility).
/// In COSY, LRE(n) returns 1 (a real always takes 1 unit).
/// In Rosy, we return 1 for compatibility.
pub trait RosyLRE {
    fn rosy_lre(&self) -> RE;
}

impl RosyLRE for RE {
    fn rosy_lre(&self) -> RE {
        1.0
    }
}

/// LLO(n) - Logical memory size estimator (COSY compatibility).
/// In COSY, LLO(n) returns 1 (a logical always takes 1 unit).
/// In Rosy, we return 1 for compatibility.
pub trait RosyLLO {
    fn rosy_llo(&self) -> RE;
}

impl RosyLLO for RE {
    fn rosy_llo(&self) -> RE {
        1.0
    }
}

/// LVE(n) - Vector memory size estimator (COSY compatibility).
/// In COSY, LVE(n) returns max(2, n) — a VE always occupies at least 2 RE words.
pub trait RosyLVE {
    fn rosy_lve(&self) -> RE;
}

impl RosyLVE for RE {
    fn rosy_lve(&self) -> RE {
        self.max(2.0)
    }
}

/// LCD(ve) - Complex DA memory size estimator (COSY compatibility).
/// Takes a VE with (order, num_vars) and returns estimated complex DA memory size.
/// In COSY, LCD(NO&NV) computes 2 * C(NO+NV, NV) — complex DA needs 2x real DA.
pub trait RosyLCD {
    fn rosy_lcd(&self) -> RE;
}

impl RosyLCD for VE {
    fn rosy_lcd(&self) -> RE {
        if self.len() < 2 {
            return 1.0;
        }
        let no = self[0] as u64;
        let nv = self[1] as u64;

        // Compute binomial coefficient (no + nv) choose nv
        // = (no+nv)! / (no! * nv!)
        let mut result: u64 = 1;
        for i in 1..=nv {
            result = result * (no + i) / i;
        }

        // Complex DA needs 2x the storage of real DA
        (2 * result) as f64
    }
}

/// LDA(ve) - DA memory size estimator (COSY compatibility).
/// Takes a VE with (order, num_vars) and returns estimated DA memory size.
/// Same computation as the number of monomials in `nv` variables up to degree `no`.
pub trait RosyLDA {
    fn rosy_lda(&self) -> RE;
}

impl RosyLDA for VE {
    fn rosy_lda(&self) -> RE {
        if self.len() < 2 {
            return 1.0;
        }
        let no = self[0] as u64;
        let nv = self[1] as u64;

        // Compute binomial coefficient (no + nv) choose nv
        // = (no+nv)! / (no! * nv!)
        let mut result: u64 = 1;
        for i in 1..=nv {
            result = result * (no + i) / i;
        }

        result as f64
    }
}
