# afnan checklist audit

source: `~/Downloads/checklist.pdf` (rosy status checks).
checked against rosy `merge/checklist-pass` (1.11.0) and libcosy `merge/checklist-pass` (1.1.2).
landed from worktrees `fix/re-da-promotion`, `fix/indexed-da-concat`, `fix/cml-dlact-lastbit` plus daprv/danf.

legend:
- **fixed complete**: marked resolved, holds
- **false green**: marked resolved, still broken
- **partial**: marked resolved, only part of the claim holds
- **false red**: unmarked / still listed as broken, actually works
- **still open**: never claimed fixed, still broken

## 1 general

| item | pdf | actual |
|---|---|---|
| unused var / `RERAN J` without type | resolved | **fixed complete**. untyped unused defaults to RE. |
| `RERAN` entropy default | resolved | **fixed complete**. default seed 0. `RANSEED` negative = os entropy. |
| `VELSET` 1-index error | resolved | **fixed complete**. 1-based. high indexes grow the vec. |
| `WRITE` uninit `Y` | open | **false red**. prints `0`. |
| functions without optional types | open | **false red**. inference works for the usual cases. |
| i/o unit as variable | open | **false red**. `READ UNIT X` then `WRITE 6 X` works. |
| case sensitivity | resolved (intentional) | **partial**. vars case-sensitive, hint works. mixed-case *builtins* (`Sin`) also work, even though the note said upper/lower only. |
| error messages not in source order | attempt | **false red**. sorted by line/col now. |
| assignment-chain type infer without `(RE)` | open (after case match) | **false red**. `K0 := GAMMA0 - 1` infers. |
| same var local + global | open | **false red**. local shadows. |
| `TYPE()` vs cosy numbers | resolved | **fixed complete**. 1..7 for RE ST LO CM VE DA CD. no GR. |
| multi-dim indexing `X(I,J)` / `X[I,J]` / `X(I)(J)` | resolved | **fixed complete**. |
| `CPUSEC` / `PWTIME` compile | resolved | **fixed complete**. cpu vs wall as designed. |
| `QS := .1` parse | resolved | **fixed complete**. |
| `LOOP I 5 1 -2` parse | resolved | **fixed complete**. |

## 2 vectors

| item | pdf | actual |
|---|---|---|
| `X := 0` then `X := X & I` | resolved (option A: use `VE(0)`) | **partial / by design**. `X := 0` is RE. original snippet still fails unless you write `VE(0)`. |
| concat non-integer after `RERAN` | resolved | **fixed complete**. concat keeps float. index rounds like cosy. |
| `X := -1` then concat `10^(-I)` | resolved (option A) | same as first. use `VE(-1)`. |
| vector display chopped exponents | resolved | **fixed complete**. sci notation preserved. |

## 3 da vectors

| item | pdf | actual |
|---|---|---|
| `COORD` concat DA vs VE | resolved via `VE(...)` | **fixed complete**. indexed concat nests one DA dim. `concat_da_indexed.rosy` passes. |
| `MAP1(1) := DA(1); MAP1(2) := DA(2)/P0` | open | **false red**. compiles. |
| `X(1) := 1` then `X(1) := X(1) + DA(1)` | open | **fixed complete**. RE→DA promotion. `re_da_promo.rosy` passes. |
| all components zero display | resolved | **fixed complete**. |

## 4 mpi

| item | pdf | actual |
|---|---|---|
| NP dim order (first vs last) | open | **false red**. last/innermost dim is rank, matching cosy. |

## 5 libcosy

| item | pdf | actual |
|---|---|---|
| GTRA/LTRA omitted | resolved | **fixed complete**. um seeds from DD. |
| DI map doubled 31.41 | resolved | **fixed complete**. ran ~15.707. |
| CMS mismatch | resolved | **fixed complete** at the integrator path (RKCO + hamiltonian). no bit-identical rerun. |
| `MQK 0.4 -1.29 0.05` arg split | resolved | **fixed complete**. `CO NOC-1` still one arg. |
| leading-dot on elements | resolved | **fixed complete**. |
| `PARA(1)` on RP / elements | resolved | **fixed complete**. |
| FR `RKLOG.DAT` | resolved | **fixed complete**. unit 77 REPLACE. |
| GT vs cosy (hyperbolic CD) | resolved | **fixed complete** for the CONS leftover. `CONS(CD)` keeps imag as CM. no `+ 0*IM`. |
| ME matrix elements | resolved | **fixed complete** (DAPEE is correct). MQ ME11/12/21/22 match the pdf. |
| DAPRV exponent order / PM 6 slots | resolved | **fixed complete**. `min(max_vars, 6)` digits. |
| CML/CMR residual | attempt then resolved | **fixed** at the DLACT rewrite (`α²−β` form). not re-run bit-for-bit vs COSY in this pass. |
| ER `step != 0` panic | resolved | **fixed complete**. |
| TP/TS DA-only vs CD | resolved | **fixed complete**. signatures are `MU (CD 4)`. |

## integration branches

rosy `merge/checklist-pass` (1.11.0) = `merge/afnan-dev` + daprv + `fix/indexed-da-concat` + `fix/re-da-promotion`.
libcosy `merge/checklist-pass` (1.1.2) = `merge/afnan-dev` + danf + `fix/cml-dlact-lastbit`.

re-verified 2026-08-27 on those branches: unused/reran, write uninit, untyped fn, infer chain, shadow, TYPE(), indexing, `.1`, CPUSEC/PWTIME, io unit var, case hint, VE option A still errors, VE display, ALL COMPONENTS ZERO, MAP1, re_da_promo, concat_da_indexed, daprv, loop, MQK negative, DI GTRA ±10, MQ ME matches pdf.

not bugs:
- option A vector shorthand (intentional).
- mixed-case builtins being accepted (more permissive than the note).

cml was not re-run bit-for-bit vs COSY here, only the DLACT rewrite landed.
