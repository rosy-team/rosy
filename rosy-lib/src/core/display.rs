use crate::taylor::{MAX_VARS, cosy_display_rank, get_config, get_runtime};
use crate::{CD, CM, DA, LO, RE, ST, VE};

const ALL_COMPONENTS_ZERO: &str = "     ALL COMPONENTS ZERO\n     -------------------";

fn sci(x: f64) -> (f64, i32) {
    if x == 0.0 {
        return (0.0, 0);
    }

    if x >= 1.0 {
        // No exponent shifting needed for your rules.
        (x, 0)
    } else {
        let exp = (-x.log10()).floor() as i32;
        let base = x * 10f64.powi(exp);
        // For exact powers of 10 (e.g. 1e-10), base rounds to exactly 1.0.
        // Normalize to [0.1, 1) by shifting down one more decade.
        if base >= 1.0 {
            (base / 10.0, -(exp - 1))
        } else {
            (base, -exp)
        }
    }
}
fn display_ve_element(x: f64) -> String {
    let sign = if x.is_sign_negative() { '-' } else { ' ' };
    let abs_x = x.abs();

    if (abs_x != 0.0 && abs_x < 0.1) || abs_x >= 1e7 {
        // Scientific: ±0.xxxxxxxE±eee  (1+2+7+5 = 15 chars)
        let (mantissa, exp) = if abs_x < 1.0 {
            sci(abs_x)
        } else {
            let e = abs_x.log10().floor() as i32 + 1;
            let m = abs_x / 10f64.powi(e);
            (m, e)
        };
        let digits: String = format!("{:.7}", mantissa).chars().skip(2).take(7).collect();
        format!("{}0.{}E{:+04}", sign, digits, exp)
    } else {
        // Fixed: sign + right-justified-10 + 4 blanks = 15 chars (G15.7 = F11.7 + 4 blanks)
        let dec_places: usize = if abs_x < 1.0 {
            7
        } else if abs_x < 10.0 {
            6
        } else if abs_x < 100.0 {
            5
        } else if abs_x < 1_000.0 {
            4
        } else if abs_x < 10_000.0 {
            3
        } else if abs_x < 100_000.0 {
            2
        } else if abs_x < 1_000_000.0 {
            1
        } else {
            0
        };
        let value_str = format!("{:.prec$}", abs_x, prec = dec_places);
        format!("{}{:>10}    ", sign, value_str)
    }
}
pub(crate) fn display_re(
    num: RE,
    precision: usize,
    exponent_precision: usize,
    spaces: usize,
) -> String {
    if num.abs() < 1f64 && num != 0f64 {
        // A value just under 0.1 still prints as `0.1000...` with no exponent
        // once it rounds at `precision` decimal places. COSY does that for the
        // beamlet radius ratio.
        let fixed = format!("{:.*}", precision, num.abs());
        if fixed.as_bytes().get(2).copied() == Some(b'1')
            || fixed.as_bytes().get(2).is_some_and(|c| *c != b'0' && fixed.starts_with("0."))
        {
            let digits: String = fixed.chars().skip(2).take(precision).collect();
            let pad = " ".repeat(spaces);
            return if num.is_sign_positive() {
                format!(" 0.{digits}{pad}")
            } else {
                format!("-.{digits}{pad}")
            };
        }
        // Round to `precision` significant digits. Scaling by `10^exp` and
        // then formatting keeps a leftover ulp, so `5e-5` prints as
        // `0.4999...E-004` instead of `0.5000...E-004`.
        let prec_after = precision.saturating_sub(1);
        let rendered = format!("{:.*e}", prec_after, num.abs());
        let (mant, exp_s) = rendered.split_once('e').unwrap_or(("0", "0"));
        let exp: i32 = exp_s.parse().unwrap_or(0) + 1;
        let mut digits: String = mant.chars().filter(|c| *c != '.').collect();
        if digits.len() < precision {
            digits.extend(std::iter::repeat('0').take(precision - digits.len()));
        } else {
            digits.truncate(precision);
        }
        let exp_str = if exp != 0 {
            format!("E{exp:+0exponent_precision$}")
        } else {
            " ".repeat(spaces)
        };
        if num.is_sign_positive() {
            format!(" 0.{digits}{exp_str}")
        } else {
            format!("-.{digits}{exp_str}")
        }
    } else {
        // 16 significant digits, then put the point back. Scaling by a power of
        // ten first drops the digit that should round up, so 4697.1863934982566
        // stayed ...256 instead of ...257.
        let prec_after = precision.saturating_sub(1);
        let rendered = format!("{:.*e}", prec_after, num.abs());
        let (mant, exp_s) = rendered.split_once('e').unwrap_or(("0", "0"));
        let exp: i32 = exp_s.parse().unwrap_or(0);
        let digits: String = mant.chars().filter(|c| *c != '.').collect();
        let int_len = (exp + 1).max(0) as usize;
        let body = if int_len >= digits.len() {
            let mut s = digits;
            s.extend(std::iter::repeat('0').take(int_len - s.len()));
            s.push('.');
            s
        } else if int_len == 0 {
            format!("0.{digits}")
        } else {
            let (i, f) = digits.split_at(int_len);
            format!("{i}.{f}")
        };
        format!(
            "{}{body}{}",
            if num.is_sign_negative() { "-" } else { " " },
            " ".repeat(spaces),
        )
    }
}
fn build_exp_str(exps: &[u8], num_vars: usize) -> String {
    exps[..num_vars.min(exps.len())]
        .iter()
        .enumerate()
        .fold(String::new(), |mut acc, (i, exp)| {
            if i % 2 == 0 {
                acc.push_str(&format!("{:>2}", exp));
            } else {
                acc.push_str(&format!("{:>2} ", exp));
            }
            acc
        })
}
pub trait RosyDisplay {
    fn rosy_display(self) -> String;
}
impl RosyDisplay for &RE {
    fn rosy_display(self) -> String {
        display_re(*self, 16, 4, 4)
    }
}

impl RosyDisplay for &ST {
    fn rosy_display(self) -> String {
        self.to_string()
    }
}

impl RosyDisplay for &LO {
    fn rosy_display(self) -> String {
        if *self { "TRUE" } else { "FALSE" }.to_string()
    }
}

impl RosyDisplay for &CM {
    fn rosy_display(self) -> String {
        // COSY format: (  real     ,  imag     )
        format!(
            " ( {}, {})",
            display_re(self.re, 9, 4, 5),
            display_re(self.im, 9, 4, 5)
        )
    }
}

impl RosyDisplay for &VE {
    fn rosy_display(self) -> String {
        self.iter()
            .map(|x| display_ve_element(*x))
            .collect::<Vec<String>>()
            .join("")
    }
}

impl RosyDisplay for &DA {
    fn rosy_display(self) -> String {
        // Output in COSY format: multi-line with all coefficients

        // Get all coefficients
        let coeffs: Vec<_> = self.coeffs_iter();
        if coeffs.is_empty() {
            return ALL_COMPONENTS_ZERO.to_string();
        }

        let mut sorted = coeffs.clone();
        let sort_vars = get_config()
            .map(|config| config.num_vars)
            .unwrap_or(MAX_VARS);
        sorted.sort_by_cached_key(|(m, _)| {
            (
                m.total_order,
                cosy_display_rank(&m.exponents, sort_vars),
                m.exponents,
            )
        });

        let mut output = String::new();
        output.push_str("I  COEFFICIENT            ORDER EXPONENTS\n");
        for (idx, (monomial, coeff)) in sorted.iter().enumerate() {
            let order = monomial.total_order;
            let exp_str = {
                let exps = &monomial.exponents;
                let nv = get_runtime()
                    .map(|rt| rt.config.num_vars)
                    .unwrap_or(exps.len());
                build_exp_str(exps, nv)
            };
            output.push_str(&format!(
                "{}  {} {}  {}\n",
                idx + 1,
                coeff.rosy_display(),
                format!("{:>3}", order),
                exp_str.trim_end()
            ));
        }

        let last_line_length = output.lines().last().unwrap_or("").len();
        output.push_str(&"-".repeat(last_line_length));
        output
            .lines()
            .map(|st| format!("     {}", st))
            .collect::<Vec<String>>()
            .join("\n")
    }
}

impl RosyDisplay for &CD {
    fn rosy_display(self) -> String {
        // Output in COSY format: multi-line with all complex coefficients

        // Get real and imaginary parts
        let real_part = self.real_part();
        let imag_part = self.imag_part();

        // Combine all monomials from both parts
        let mut all_monomials = std::collections::HashSet::new();
        for (m, _) in real_part.coeffs_iter() {
            all_monomials.insert(m);
        }
        for (m, _) in imag_part.coeffs_iter() {
            all_monomials.insert(m);
        }

        if all_monomials.is_empty() {
            return ALL_COMPONENTS_ZERO.to_string();
        }

        let mut sorted: Vec<_> = all_monomials.into_iter().collect();
        let sort_vars = get_config()
            .map(|config| config.num_vars)
            .unwrap_or(MAX_VARS);
        sorted.sort_by_cached_key(|m| {
            (
                m.total_order,
                cosy_display_rank(&m.exponents, sort_vars),
                m.exponents,
            )
        });

        let mut output = String::new();
        output.push_str("     I  COEFFICIENTS                           ORDER EXPONENTS\n");
        for (idx, monomial) in sorted.iter().enumerate() {
            let real_coeff = real_part.get_coeff(monomial);
            let imag_coeff = imag_part.get_coeff(monomial);
            let order = monomial.total_order;
            let exp_str = {
                let exps = &monomial.exponents;
                let nv = get_runtime()
                    .map(|rt| rt.config.num_vars)
                    .unwrap_or(exps.len());
                build_exp_str(exps, nv)
            };
            output.push_str(&format!(
                "     {} {} {} {:>3}  {}\n",
                idx + 1,
                real_coeff.rosy_display(),
                imag_coeff.rosy_display(),
                order,
                exp_str.trim_end()
            ));
        }
        output.push_str("                                      ");
        output
    }
}

// Required as loops cast to `usize`
impl RosyDisplay for &usize {
    fn rosy_display(self) -> String {
        self.to_string()
    }
}

impl RosyDisplay for &str {
    fn rosy_display(self) -> String {
        self.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::RosyDisplay;

    #[test]
    fn vector_display_keeps_small_value_exponents() {
        let values = vec![0.546920369e-2, 0.937875496e-10];
        let displayed = values.rosy_display();

        assert!(displayed.contains(" 0.5469204E-002"), "got: {displayed:?}");
        assert!(displayed.contains(" 0.9378755E-010"), "got: {displayed:?}");
    }

    #[test]
    #[serial_test::serial]
    fn zero_da_display_matches_cosy() {
        crate::taylor::cleanup_taylor();
        crate::taylor::init_taylor(2, 2).unwrap();
        let value = crate::DA::zero();

        assert_eq!(
            value.rosy_display(),
            "     ALL COMPONENTS ZERO\n     -------------------"
        );
        crate::taylor::cleanup_taylor();
    }
}
