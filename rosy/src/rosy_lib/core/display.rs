use crate::rosy_lib::{RE, ST, LO, CM, VE, DA, CD};

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
        let digits: String = format!("{:.7}", mantissa)
            .chars().skip(2).take(7)
            .collect();
        format!("{}0.{}E{:+04}", sign, digits, exp)
    } else {
        // Fixed: sign + right-justified-10 + 4 blanks = 15 chars (G15.7 = F11.7 + 4 blanks)
        let dec_places: usize = if abs_x < 1.0 { 7 }
            else if abs_x < 10.0 { 6 }
            else if abs_x < 100.0 { 5 }
            else if abs_x < 1_000.0 { 4 }
            else if abs_x < 10_000.0 { 3 }
            else if abs_x < 100_000.0 { 2 }
            else if abs_x < 1_000_000.0 { 1 }
            else { 0 };
        let value_str = format!("{:.prec$}", abs_x, prec=dec_places);
        format!("{}{:>10}    ", sign, value_str)
    }
}
fn display_re (
    num: RE,
    precision: usize,
    exponent_precision: usize,
    spaces: usize
) -> String {
    if num.abs() < 1f64 && num != 0f64 {
        let (mantissa, exponent) = sci(num.abs());

        if num.is_sign_positive() {
            format!(
                " 0.{}{}",
                format!("{:.precision$}", mantissa, precision=precision)
                    .chars()
                    .skip(2) // Skip "0."
                    .take(precision)
                    .collect::<String>(),
                if exponent != 0 {
                    format!(
                        "E{:+0exponent_precision$}",
                        exponent,
                        exponent_precision=exponent_precision
                    )
                } else {
                    " ".repeat(spaces)
                }
            )
        } else {
            format!(
                "-.{}{}",
                format!("{:.precision$}", mantissa, precision=precision)
                    .chars()
                    .skip(2) // Skip "0."
                    .take(precision)
                    .collect::<String>(),
                if exponent != 0 {
                    format!(
                        "E{:+0exponent_precision$}",
                        exponent,
                        exponent_precision=exponent_precision
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
            if num.is_sign_negative() {"-"} else {" "},
            format!(
                "{:.precision$}",
                rounded_num.abs(),
            ).chars().take(precision + 1).collect::<String>(),
            " ".repeat(spaces),
        )
    }
}
fn build_exp_str (
    exps: &[u8],
) -> String {
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
            return "     I  COEFFICIENT            ORDER EXPONENTS\n     1   0.000000000000000       0   0 0\n     -----------------------------------".to_string();
        }
        
        // Sort by graded reverse lexicographic order (COSY format)
        // First by total order, then by exponents in reverse lexicographic order
        let mut sorted = coeffs.clone();
        sorted.sort_by(|(m1, _), (m2, _)| {
            m1.total_order.cmp(&m2.total_order)
                .then_with(|| {
                    // Reverse lexicographic: compare from right to left
                    for i in (0..m1.exponents.len()).rev() {
                        match m1.exponents[i].cmp(&m2.exponents[i]) {
                            std::cmp::Ordering::Equal => continue,
                            ord => return ord,
                        }
                    }
                    std::cmp::Ordering::Equal
                })
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
        output.lines()
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
            return "     I  COEFFICIENTS                           ORDER EXPONENTS\n     1  0.000000000000000      0.000000000000000       0   0 0\n                                      ".to_string();
        }
        
        // Sort by graded reverse lexicographic order (COSY format)
        // First by total order, then by exponents in reverse lexicographic order
        let mut sorted: Vec<_> = all_monomials.into_iter().collect();
        sorted.sort_by(|m1, m2| {
            m1.total_order.cmp(&m2.total_order)
                .then_with(|| {
                    // Reverse lexicographic: compare from right to left
                    for i in (0..m1.exponents.len()).rev() {
                        match m1.exponents[i].cmp(&m2.exponents[i]) {
                            std::cmp::Ordering::Equal => continue,
                            ord => return ord,
                        }
                    }
                    std::cmp::Ordering::Equal
                })
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

    #[test]
    fn vector_display_keeps_small_value_exponents() {
        let values = vec![0.546920369e-2, 0.937875496e-10];
        let displayed = values.rosy_display();

        assert!(displayed.contains(" 0.5469204E-002"), "got: {displayed:?}");
        assert!(displayed.contains(" 0.9378755E-010"), "got: {displayed:?}");
    }
}
