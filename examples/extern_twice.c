/* EXTERN test: b := 2*a, total := sum(a), n := number of arrays. */
void twice(long long count, double **v, const long long *len) {
    double sum = 0.0;
    for (long long i = 0; i < len[0]; i++) { v[1][i] = 2.0 * v[0][i]; sum += v[0][i]; }
    v[2][0] = sum;
    v[3][0] = (double)count;
}
