use crate::rosy_lib::taylor::{MAX_VARS, cosy_display_rank, get_config};
use crate::rosy_lib::{CD, CM, DA, LO, RE, ST, VE};

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
fn display_re(num: RE, precision: usize, exponent_precision: usize, spaces: usize) -> String {
    if num.abs() < 1f64 && num != 0f64 {
        let (mantissa, exponent) = sci(num.abs());

        if num.is_sign_positive() {
            format!(
                "0.{}{}",
                format!("{:.precision$}", mantissa, precision = precision)
                    .chars()
                    .skip(2) // Skip "0."
                    .take(precision)
                    .collect::<String>(),
                if exponent != 0 {
                    format!(
                        "E{:+0exponent_precision$}",
                        exponent,
                        exponent_precision = exponent_precision
                    )
                } else {
                    " ".repeat(spaces)
                }
            )
        } else {
            format!(
                "-.{}{}",
                format!("{:.precision$}", mantissa, precision = precision)
                    .chars()
                    .skip(2) // Skip "0."
                    .take(precision)
                    .collect::<String>(),
                if exponent != 0 {
                    format!(
                        "E{:+0exponent_precision$}",
                        exponent,
                        exponent_precision = exponent_precision
                    )
                } else {
                    " ".repeat(spaces)
                }
            )
        }
    } else {
        // Round at the last visible digit
        let num_int_chs = num.trunc().to_string().chars().count();
        let rounded_num = (num * 10f64.powi(precision as i32 - num_int_chs as i32 + 1)).round()
            / 10f64.powi(precision as i32 - num_int_chs as i32 + 1);

        format!(
            "{}{}{}",
            if num.is_sign_negative() { "-" } else { " " },
            format!("{:.precision$}", rounded_num.abs(),)
                .chars()
                .take(precision + 1)
                .collect::<String>(),
            " ".repeat(spaces),
        )
    }
}
fn build_exp_str(exps: &[u8]) -> String {
    exps.iter()
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
        display_re(*self, 16, 3, 4)
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
        let elements: Vec<String> = self
            .iter()
            .map(|x| format!(" {}", display_re(*x, 9, 4, 5)))
            .collect();
        format!("{}", elements.join("     "))
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

        // Sort in COSY display order.
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
                // For 6 exponents, should match: '1 0  1 0  0 0'
                let exps = &monomial.exponents;

                build_exp_str(exps)
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

        // Sort in COSY display order.
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
                // For 6 exponents, should match: '1 0  1 0  0 0'
                let exps = &monomial.exponents;

                build_exp_str(exps)
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
    use serial_test::serial;

    #[test]
    fn vector_display_keeps_small_value_exponents() {
        let values = vec![0.546920369e-2, 0.937875496e-10];
        let displayed = values.rosy_display();

        assert!(displayed.contains("0.546920369E-002"));
        assert!(displayed.contains("0.937875496E-010"));
    }

    #[test]
    #[serial]
    fn zero_da_display_matches_cosy() {
        crate::rosy_lib::taylor::cleanup_taylor();
        crate::rosy_lib::taylor::init_taylor(2, 2).unwrap();
        let value = crate::rosy_lib::DA::zero();

        assert_eq!(
            value.rosy_display(),
            "     ALL COMPONENTS ZERO\n     -------------------"
        );
        crate::rosy_lib::taylor::cleanup_taylor();
    }
}
