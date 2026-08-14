/// Converts a real number to a string using a Fortran-style format specifier.
///
/// Supports common Fortran format descriptors:
/// - `F w.d` — fixed-point, width `w`, `d` decimal places
/// - `E w.d` — scientific notation
/// - `G w.d` — general (fixed or scientific depending on value)
/// - `I w`   — integer format
/// - `A`     — string (pass-through for the number as default string)
///
/// The format string may be enclosed in parentheses, e.g. `"(F10.3)"`.
pub fn rosy_recst(value: f64, format: &str) -> String {
    let fmt = format.trim();
    // Strip optional outer parentheses
    let fmt = if fmt.starts_with('(') && fmt.ends_with(')') {
        &fmt[1..fmt.len() - 1]
    } else {
        fmt
    };
    let fmt = fmt.trim();

    // Parse format descriptor
    let upper = fmt.to_uppercase();

    if upper.starts_with('F') {
        // Fixed-point: Fw.d
        parse_f_format(&upper[1..], value)
    } else if upper.starts_with('E') {
        // Scientific: Ew.d
        parse_e_format(&upper[1..], value)
    } else if upper.starts_with('G') {
        // General: Gw.d
        parse_g_format(&upper[1..], value)
    } else if upper.starts_with('I') {
        // Integer: Iw
        parse_i_format(&upper[1..], value)
    } else if upper.starts_with('A') {
        // String: just format as default
        format!("{}", value)
    } else {
        // Fallback: default Rust formatting
        format!("{}", value)
    }
}

fn parse_width_decimals(spec: &str) -> (usize, usize) {
    let parts: Vec<&str> = spec.split('.').collect();
    let width = parts.first()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let decimals = parts.get(1)
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(6);
    (width, decimals)
}

fn parse_f_format(spec: &str, value: f64) -> String {
    let (width, decimals) = parse_width_decimals(spec);
    if width > 0 {
        format!("{:>width$.decimals$}", value, width = width, decimals = decimals)
    } else {
        format!("{:.decimals$}", value, decimals = decimals)
    }
}

fn parse_e_format(spec: &str, value: f64) -> String {
    let (width, decimals) = parse_width_decimals(spec);
    if width > 0 {
        format!("{:>width$.decimals$E}", value, width = width, decimals = decimals)
    } else {
        format!("{:.decimals$E}", value, decimals = decimals)
    }
}

fn parse_g_format(spec: &str, value: f64) -> String {
    let (_width, decimals) = parse_width_decimals(spec);
    let abs_val = value.abs();
    // Use fixed-point if the value is in a reasonable range, otherwise scientific
    if abs_val == 0.0 || (abs_val >= 0.1 && abs_val < 10f64.powi(decimals as i32)) {
        parse_f_format(spec, value)
    } else {
        parse_e_format(spec, value)
    }
}

fn parse_i_format(spec: &str, value: f64) -> String {
    let width = spec.trim().parse::<usize>().unwrap_or(0);
    let int_val = value as i64;
    if width > 0 {
        format!("{:>width$}", int_val, width = width)
    } else {
        format!("{}", int_val)
    }
}
