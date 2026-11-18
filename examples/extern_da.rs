//! For examples/extern_da_rust.fox, the Rust twin of extern_da.c: reads DA
//! cells, writes one back. Each cell is 1 + M doubles: a flag (1 = a DA),
//! then the coefficients by degree, higher powers of earlier variables first.
pub fn da_demo(v: &mut [&mut [f64]]) {
    let cells = v[1][0] as usize;
    let block = v[0].len() / cells;
    // report the order: where the coefficients 2, 3, 5 of cell 1 sit
    for k in 0..block - 1 {
        if v[0][1 + k] != 0.0 {
            v[2][k] = v[0][1 + k];
        }
    }
    // cell 2 was not a DA: make it 1 + 7*x1*x3 (degree 2, third entry)
    let two = &mut v[0][block..2 * block];
    two[0] = 1.0;
    two[1] = 1.0;
    two[1 + 6] = 7.0;
}
