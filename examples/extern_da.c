/* For examples/extern_da.fox: reads DA cells, writes one back.
 * Each cell is 1 + M doubles: a flag (1 = a DA), then the coefficients by
 * degree, higher powers of earlier variables first. */
void da_demo(long long count, double **v, const long long *len) {
    const long long cells = (long long)v[1][0], block = len[0] / cells;
    double *c = v[0];
    /* report the order: where the coefficients 2, 3, 5 of cell 1 sit */
    for (long long k = 0; k < block - 1; k++)
        if (c[1 + k] != 0.0) v[2][k] = c[1 + k];
    /* cell 2 was not a DA: make it 1 + 7*x1*x3 (degree 2, third entry) */
    double *two = c + block;
    two[0] = 1.0;
    two[1] = 1.0;
    two[1 + 6] = 7.0;
    (void)count;
}
