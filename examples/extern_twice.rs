//! EXTERN test in Rust (examples/extern_rust.fox): b := 2*a, total := sum(a),
//! n := number of arrays. Same as extern_twice.c.
pub fn twice(v: &mut [&mut [f64]]) {
    let count = v.len() as f64;
    let sum: f64 = v[0].iter().sum();
    for i in 0..v[0].len() {
        v[1][i] = 2.0 * v[0][i];
    }
    v[2][0] = sum;
    v[3][0] = count;
}
