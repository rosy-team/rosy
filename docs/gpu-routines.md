# GPU acceleration without changing user code

Existing COSY scripts stay as they are, including `cosy.fox`. Rosy only replaces the internals of routines those scripts already call. If a call is too small for the GPU, it keeps today’s CPU path.

## What we would speed up

| Routine | What it is | When the GPU is used |
|---|---|---|
| Series multiply | The product of two Taylor series. Sits under every magnet, every `SIN`/`EXP` of a series, and every `POLVAL`. | One product is large (high order, many variables). |
| `SIN`, `COS`, `SINH`, `COSH`, `SQRT`, `EXP`, `LOG` of a series | How a quadrupole’s closed form is actually computed (`COS` and `SIN` of the focusing strength). | That one series is large. |
| `POLVAL` on series | Composes one map with another. Used inside COSY for map algebra. | That one composition is large. |
| `POLVAL` on a list of points | Applies one finished map to many rays in a single call. | That call already contains many rays. |

A loop that calls `POLVAL` once per ray is not combined into one GPU launch. Doing that would mean changing the program, which we are not doing.

## What we are not doing

New physics features (particle–particle forces, multipole translation, adaptive integrators, GPU linear algebra). They are not on the path of current scripts unless those scripts are rewritten.

The optimizer (`FIT`, simplex, Levenberg–Marquardt) and the Runge–Kutta coefficient table are not GPU work. The table is built once. The optimizer is a few numbers around a slow objective; the objective gets faster only because the routines above get faster.

## Order

1. Series multiply.
2. The series versions of `SIN`, `COS`, `SINH`, `COSH`, and `SQRT`, because the quadrupole closed form is those calls.
3. `POLVAL` on series.
4. `POLVAL` on a long list of points.

Items 1–3 help current lattice runs only at high order. At the orders most scripts use, one series is too small and the CPU path remains. Item 4 helps only scripts that already pass many rays in one `POLVAL`.
