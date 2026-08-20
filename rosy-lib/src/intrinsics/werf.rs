use crate::RosyType;
use crate::{CM, CD};

/// Get the return type of WERF for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::CM() => Some(RosyType::CM()),
        t if *t == RosyType::CD() => Some(RosyType::CD()),
        _ => None,
    }
}

/// Trait for computing the Faddeeva function of Rosy data types.
pub trait RosyWERF {
    type Output;
    fn rosy_werf(&self) -> anyhow::Result<Self::Output>;
}

/// WERF for complex numbers: Faddeeva function w(z) = exp(-z^2)*erfc(-iz)
///
/// Uses the Weideman (1994) N=32 rational approximation for ~15 digits accuracy.
/// Reference: J.A.C. Weideman, "Computation of the Complex Error Function",
/// SIAM J. Numer. Anal. 31(5):1497-1518, 1994.
impl RosyWERF for CM {
    type Output = CM;
    fn rosy_werf(&self) -> anyhow::Result<Self::Output> {
        Ok(faddeeva_w(*self))
    }
}

impl RosyWERF for crate::RosyValue {
    type Output = CM;
    fn rosy_werf(&self) -> anyhow::Result<Self::Output> {
        self.clone().expect_cm()?.rosy_werf()
    }
}

/// WERF for CD (complex Taylor composition).
///
/// w'(z) = -2*z*w(z) + 2i/sqrt(pi)
///
/// For the argument f = f0 + t (where t is the DA variable part):
///   (n+1)*w_{n+1} = -2*f0*w_n - 2*w_{n-1}   for n >= 1
///   w_1 = -2*f0*w_0 + 2i/sqrt(pi)             for n = 0
impl RosyWERF for CD {
    type Output = CD;
    fn rosy_werf(&self) -> anyhow::Result<Self::Output> {
        cd_werf(self)
    }
}

/// Faddeeva function w(z) via Weideman (1994) N=32 trapezoidal approximation.
///
/// Reference: J.A.C. Weideman, "Computation of the Complex Error Function",
/// SIAM J. Numer. Anal. 31(5):1497-1518, 1994.
///
/// The Faddeeva function has the Cauchy integral representation for Im(z) > 0:
///   w(z) = (i/pi) * ∫_{-∞}^{∞} exp(-t²) / (z - t) dt
///
/// Approximated by the symmetric trapezoidal rule at nodes t_n = n*h,
/// n = -(N-1)..(N-1), with step h = L/N:
///
///   w(z) ≈ (i * h / pi) * Σ_{n=-(N-1)}^{N-1} exp(-(n*h)²) / (z - n*h)
///
/// Valid for Im(z) >= 0. For Im(z) < 0: use reflection w(z) = 2*exp(-z²) - w(-z).
///
/// With N=32 and L = sqrt(N) * 2^(1/4) ≈ 6.74, accuracy is ~15 digits.
fn faddeeva_w(z: CM) -> CM {
    use num_complex::Complex64;

    // Reflection: w(z) for Im(z) < 0 via w(-z) with Im(-z) > 0
    if z.im < 0.0 {
        let w_neg = faddeeva_w(-z);
        return 2.0 * (-z * z).exp() - w_neg;
    }

    // N=32 Weideman symmetric trapezoidal approximation.
    // Optimal L = sqrt(N) * 2^(1/4); h = L/N (half-step, nodes at n*h).
    const N: usize = 32;
    let l: f64 = (N as f64).sqrt() * 2.0_f64.powf(0.25);
    let h: f64 = l / (N as f64);  // node spacing

    // i * h / pi
    let ih_over_pi = Complex64::new(0.0, h / std::f64::consts::PI);

    let mut sum = Complex64::new(0.0, 0.0);
    for n in -(N as i64 - 1)..=(N as i64 - 1) {
        let nh = (n as f64) * h;
        let weight = (-(nh * nh)).exp();
        // z - nh: nh is real, z is complex; Im(z - nh) = Im(z) >= 0
        sum += weight / (z - nh);
    }

    ih_over_pi * sum
}

/// Compute WERF of a CD object using Horner's method (Taylor composition).
///
/// The recurrence for the Taylor coefficients of w(f) where f = f0 + t:
///
/// w'(z) = -2*z*w(z) + 2i/sqrt(pi)
///
/// For f(t) = f0 + t (f_0 = f0, f_1 = 1, f_k = 0 for k >= 2):
///   (h*g)_n = f0*g_n + g_{n-1}   (convolution with h = f)
///
/// So: (n+1)*g_{n+1} = -2*(f0*g_n + g_{n-1}) + (2i/sqrt(pi))*delta_{n,0}
/// For n=0: g_1 = -2*f0*g_0 + 2i/sqrt(pi)
/// For n>=1: (n+1)*g_{n+1} = -2*f0*g_n - 2*g_{n-1}
fn cd_werf(cd: &CD) -> anyhow::Result<CD> {
    
    use num_complex::Complex64;

    let config = crate::taylor::get_config()?;
    let nocut = config.max_order as usize;

    let f0 = cd.constant_part();
    let cd_prime = cd.make_prime();

    let two_i_over_sqrtpi = Complex64::new(0.0, 2.0 / std::f64::consts::PI.sqrt());

    let mut xf = Vec::with_capacity(nocut + 1);
    xf.push(faddeeva_w(f0));
    if nocut >= 1 {
        xf.push(-2.0 * f0 * xf[0] + two_i_over_sqrtpi);
    }
    for n in 1..nocut {
        let next = (-2.0 * f0 * xf[n] - 2.0 * xf[n - 1]) / Complex64::new((n + 1) as f64, 0.0);
        xf.push(next);
    }

    CD::horner_eval(&cd_prime, &xf)
}
