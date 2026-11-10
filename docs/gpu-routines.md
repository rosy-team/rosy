# GPU acceleration without changing user code

Existing COSY scripts stay as they are, including `cosy.fox`. Rosy only replaces the internals of routines those scripts already call. A call that is too small for the GPU keeps today's CPU path.

The numbers below are estimates from flop counts and memory traffic, not from a GPU run. A multiplier is today's single-threaded wall time divided by the time with that path. Above 1 is faster.

Two populations:

- **Bottleneck runs.** Runs where this routine is already most of the time. Bad is about the slowest of those in a hundred (1st percentile). Best is about the fastest in a hundred (99th percentile). Typical is the middle.
- **All runs.** A normal lattice mix. This routine is usually not the bottleneck, and the CPU fallback then changes nothing.

## Scoring

Each idea gets a score from about 0 to 10. Higher is more worth doing.

```
score = 0.5 * t + 0.3 * b + 0.2 * w
```

`t`, `b`, and `w` turn the bottleneck typical, best, and bad multipliers `m` into points. For `m >= 1` the points are `min(10, 4 * log2(m))`. For `m < 1` the points are `4 * log2(m)`, which is negative. Half the score is the speedup on a run this routine already dominates, 30 percent is the best of those runs, and 20 percent is the penalty when the bad run gets slower.

## Rank

| Rank | Score | Idea | Bad | Typical | Best | All runs |
|---|---|---|---|---|---|---|
| 1 | 6.6 | Many-ray `POLVAL` | 0.7x | 4x | 10x | ~1.1x |
| 2 | 6.2 | High-order map step, kept on the device | 0.5x | 4x | 10x | ~1.0x |
| 3 | 5.2 | High-order map composition | 0.7x | 2.5x | 6x | ~1.0x |

### 1. Many-ray `POLVAL` (6.6)

Best target. `POLVAL` already applies one map to every ray in the call. `SR` appends with `&`, and tracking uses `POLVAL 1 MSC ... RAY 8 RAY`. Each of the eight `RAY` columns holds every particle (the buffer is 10,001 long). The GPU path is one Horner kernel, parallel across rays. The map is small and uploads once per element. The rays have to stay on the device across elements.

The best case is about 10,000 rays, order around 5, pictures off, so nothing copies the rays back each element. About 10x against one core. Several CPU threads on the same Horner loop would take most of that, down to roughly 3x, because the loop is limited by memory bandwidth. The 4-wide `nightly-simd` path is the same idea at a smaller width.

The bad case is a few hundred rays with pictures on. `LPI` reads `TRA` on the CPU every element, the rays cross the bus every element, and the kernel loses: about 0.7x on that call. The default picture count is 251 and should stay on the CPU.

Most runs never get here. Order 2 or 3 with a handful of rays (envelope, `SCDE`) stays on the CPU, so a random lattice run is about 1.1x.

### 2. High-order map step, kept on the device (6.2)

When `NOC >= 3`, or fringe fields or spin are on, a quadrupole does not use the closed form. `FFELE` propagates the map with `DAFLO` (no fringe, `SIELE`) or with Runge–Kutta (`SDELE`). Both are a chain of series multiplies and derivatives. The GPU work is that whole chain in one launch, coefficient buffers left on the device. One launch per multiply is slower than the CPU, so a bare multiply kernel is not a separate idea.

Worth it only when the series is large and fairly dense. Order 7 with 6 variables is 1,716 monomials. Order 9 with 6 variables is 5,005, and the multiply table is about 100MB, still under the 256MB cutoff. Order 11 with 6 variables is 12,376 monomials and the table is about 600MB, so Rosy never builds it. Those products use exponent arithmetic and a hash map. Leave them on the CPU.

The inner multiply is a scatter through the table, about one multiply-add per 4-byte lookup. The best case, a dense order-9 flow that is almost the whole run, is about 10x against one core. Early Lie terms are sparse, and the CPU skips zeros, so a dense GPU walk of a sparse series is the bad case, about 0.5x. Extra CPU cores take less of this win than they do for rays, because each step waits on the previous one. A random lattice run does not change: low-order elements use the closed form and never call this.

### 3. High-order map composition (5.2)

Series `POLVAL` composes one map with another (`ANM`, symplectic scaling, normal-form algebra). It is a Horner chain of full series products, one node at a time. Same device-side multiply as rank 2, but the chain is as long as the monomial list, so more of the time is waiting between steps.

The best case is repeated composition at order 7 to 9: about 6x against one core, and about 3x as a whole run because other series work sits around it. Low-order composition stays on the CPU. A random run is unchanged.

This shares its kernel with rank 2. It is not a third project.

## Left out

Series multiply as its own kernel. Real `SIN`, `COS`, `SINH`, `COSH`, `EXP`, `LOG`, and `SQRT` do not call it. They use a degree-by-degree convolution in `compose.rs`. A single product is also too small to launch, and at the orders where the product is fat the multiply table is often absent.

`SIN` and `COS` of one series, and the quadrupole closed form as a reason to do them. That closed form in `MQ` runs only for `NOC < 3`, no fringe, no spin. The focusing strength there is a real number unless the length or the field is itself a series, so `COS` and `SIN` are scalar. The series operation on that path is a square root of the kinematic factor, at an order too small to launch. High-order magnets take rank 2.

`EXP` and `LOG` as their own GPU routines. Same small recurrence, and they are not the quadrupole formula.

The `FIT` driver, simplex, Levenberg–Marquardt, and the Runge–Kutta coefficient table. The table is built once. The optimizers are a few numbers around an objective. The objective gets faster only when that objective actually calls rank 1 or rank 2.

Fusing a script loop that calls `POLVAL` once per ray. That would change the program. The batched call is already how `cosy.fox` tracks.
