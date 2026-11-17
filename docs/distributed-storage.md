# Distributed particle storage for PHAD

Status: build steps 1 to 6 are done (`rosy-lib/src/fmmtree.rs`, `rosy-lib/src/pstore.rs`) and a port of PHAD runs on them. Parallel input and output remain.

## Goal

Run PHAD at 10^9 particles on Metis (32 nodes, 128 cores and 1.3 TB each).
No rank may hold or loop over anything of size N. Every per-rank cost must be
O(N/P) plus a thin boundary layer, so adding nodes adds both capacity and speed.

## What breaks today

Measured on the 50k, 10-step deck (pinned cores, shared nodes):

| | 128 ranks | 256 ranks |
|---|---|---|
| Inside parallel loops, per rank | 147 s | 70 s |
| Serial code between loops, per rank | 123 s | 174 s |
| Wall | 376 s | 423 s |

The parallel work halves. The replicated work grows, and the run gets slower.
The causes, from `phad.fox` and fmmcpp:

- Every rank holds all eight particle arrays, every multipole and local
  expansion, and gather arrays of shape `q x leaves x NP`.
- Every rank runs its own fmmcpp, which reads all positions and builds the
  whole tree. The tree reaches PHAD through files at 48 bytes per number.
- fmmcpp deals boxes to ranks by sorted cost, so one rank's boxes are
  scattered through space and it needs data from everywhere.
- After each gather, every rank runs the same O(N) loop to copy results into
  its own global arrays. The costliest is the stitching of previous-neighbour
  lists after `phad.fox:1002`: 85 s at 128 ranks, 128 s at 256.

## The design in one paragraph

Each particle gets a 60-bit Morton key from its scaled position, the same key
fmmcpp computes. Each rank owns one contiguous key range and keeps its
particles sorted by key. A leaf box is then a contiguous slice of a rank's
arrays, the tree is implied by key prefixes, and "who owns this box" is a
binary search over P-1 splitter keys. Ranks exchange only a boundary layer.

## 1. Particle record

Struct of arrays, one entry per owned particle, sorted by key:

| Field | Size | Notes |
|---|---|---|
| `id` | 8 B | Original particle number. Never changes. |
| `key` | 8 B | 3 x 20 bits, x then y then z, as in fmmcpp's `giveBoxNumber`. |
| `x y z px py pz f n` | 64 B | PHAD's `source_*` arrays. |
| `e` | 8 B | A per-particle tag the program owns; the store moves it with the particle and never reads it. PHAD's Nov 2026 version keeps each particle's lattice element there (`source_element`), which its aperture lookup starts from, so it must survive migration and halo copies. |
| `prev_leaf` | 8 B | Leaf (level, number) the particle was in last step. |

96 bytes per particle. Slots are ordered by `(key, id)`, so any box at any
level is a contiguous range. Source lists handed to PHAD are sorted by `id`
per box, so sums run in the same order as today.

## 2. Ownership

- P-1 splitter keys `s_1 < ... < s_{P-1}`. Rank r owns keys in `[s_r, s_{r+1})`.
- Splitters come from a weighted sample of keys, where the weight is the
  estimated cost of the particle's leaf. This replaces fmmcpp's sorted-cost
  deal with a partition that is balanced and spatially compact.
- After leaves are known, each splitter is moved to the start of the leaf it
  falls in, so no leaf straddles two ranks.
- Splitters are rebalanced every few steps, not every step.

Every rank knows all splitters (P-1 numbers), so finding the owner of any key
or box needs no directory and no communication.

## 3. Tree

A linear octree: no pointers, no global box numbering.

- A box is `(level, number)` where `number = key >> 3*(20 - level)`.
- fmmcpp's refinement rule is kept exactly: a box holding targets is a leaf
  when the 27 same-level boxes around it hold at most `q` sources; otherwise
  it and its neighbours divide.
- The rule needs only per-box particle counts. Each rank histograms its own
  keys per level and asks neighbouring ranks for counts of boxes just outside
  its range. One round per level, about 7 to 10 rounds in total.
- Leaves are stored as `(level, number, first, count)`: 24 bytes per leaf.
- Parents, children, neighbours and interaction lists are computed from the
  box number on demand, in fmmcpp's order, and never stored globally.

## 4. Boundary layer (halo)

For each leaf near the edge of its range, a rank works out the 26 neighbour
boxes, finds their owners from the splitters, and requests their particles.
Halo particles are appended after the owned ones in the same arrays.

This is what lets PHAD's loops survive nearly unchanged: `source_x(INDEX)`
still works, but `INDEX` is now a local position and the `snumbs` / `nnumbs`
lists hold local positions. The gathers after the near-field and long-range
loops, and the scatter loops that follow them, are deleted: results are
written straight into the owned slice.

## 5. Expansions

- A box's multipole and local expansion live on the rank that owns the box's
  first key. Storage is a per-rank table keyed by `(level, number)`.
- Coefficients are stored dense: 84 doubles, 672 bytes, no index list.
- Upward pass: a box whose children sit on another rank receives their
  contributions by message. Only boxes spanning a splitter are affected.
- Downward pass: a rank fetches the multipoles of interaction-list boxes it
  does not own. With compact ranges this is a surface layer about three boxes
  deep per level.
- Local expansions are kept only for the level in progress and its parent
  level, then for leaves.

## 6. Previous neighbourhoods

The long-range integrator compares each particle's neighbourhood with the one
it had last step. Today that is a global list on every rank.

New form: each rank keeps, for each leaf it owned last step, the list of
particle ids in that leaf's neighbourhood (at most `q` ids). A particle carries
`prev_leaf`. The rank that owns the particle now fetches that list from last
step's owner, found with last step's splitters. Then it fetches positions for
the ids that are no longer in its halo.

## 7. One step

1. Near-field integration over owned leaves, using owned plus halo particles.
2. Global bounding box by reduction; recompute keys (z is scaled by `relg`).
3. Local sort, then send particles whose key left the range to the new owner.
4. Refinement rounds; snap splitters; build leaf table.
5. Halo exchange.
6. Upward pass, multipole fetch, downward pass level by level.
7. Long-range kick over owned leaves.
8. Global sums (`relg`) by an order-independent exact reduction.

## 8. Memory at 10^9 particles

About 0.7 leaves and 2 boxes per particle, from the 50k deck.

| Item | Per particle | Total |
|---|---|---|
| Particle record | 88 B | 0.09 TB |
| Halo copies (estimate) | 30 B | 0.03 TB |
| Leaf table | 17 B | 0.02 TB |
| Multipoles | 730 B | 0.73 TB |
| Local expansions (leaf level plus one working level) | 600 B | 0.60 TB |
| Previous-neighbourhood lists | 340 B | 0.34 TB |
| **Total** | **about 1.8 KB** | **about 1.8 TB** |

Today's projection is 326 TB at 35 ranks. At 4096 ranks the new layout is
about 0.45 GB per rank on top of the fixed 0.37 GB a rank uses at start.

## 9. Agreement with current results

- Same keys, same refinement rule, same leaf set as fmmcpp. This is testable
  box for box against fmmcpp's files.
- Same summation order inside each leaf and each interaction list.
- `relg` and other global sums change in the last bits, because a sum over a
  distributed array cannot follow index order. COSY already differs at 1e-15
  between builds, so the target is agreement to that level.

## 10. Build order

1. Done. Keys, refinement and leaf table in plain Rust on one process. The
   leaf set, box centres, source lists and neighbour lists match fmmcpp
   exactly at 5k (3,502 leaves) and 50k (33,183 leaves).
2. Done. Splitters, distributed sort and migration.
3. Done. Count exchange and halo exchange. `examples/pstore_check.rs` checks
   leaves, their particles and their neighbour lists against the
   single-process tree at any rank count.
4. PHAD's near-field loop on the new storage, checked against the current run.
5. Expansion tables and the upward and downward passes.
   - Done on one process: `LeafTable::plan`, `interaction_list` and
     `multipole_recipe` reproduce fmmcpp's D-tree, interaction lists (in
     order) and upward-pass structure exactly at 5k and 50k.
   - Done across ranks: `ParticleStore::build_plan` gives each box to the
     rank that holds its first key, and `examples/pstore_check.rs` checks
     every interaction list and multipole recipe against the one-process
     plan at any rank count.
   - Done: per-level work lists (`PSUP`, `PSDOWN`) and exchange of
     expansions between ranks (`PSMSYNC`, `PSLSYNC`).
     `examples/pstore_fmm.fox` runs both passes with PHAD's own expansion
     procedures. Its potentials are the same at 1, 2, 4 and 7 ranks, and
     within 6e-5 of the direct sum at order 6.
6. Done. Previous-neighbourhood lists (`PSPREV`, `PSPOS`), halo refresh
   (`PSHALO`), global sums (`PSSUM`) and gathered output (`PSALL`). A port
   of `phad.fox` onto the store gives output files byte-identical to the
   file-based version at 5k (32 ranks) and 50k (32, 64 and 72 ranks), with
   `ROSY_PS_ORDERED_SUM` set.
7. To do: parallel input and output. The first rank still reads the input
   files and writes the output files.

## Lessons from the port

- **Sums.** `PSSUM` returns the correctly rounded total, which does not
  depend on rank count but is not what a one-process loop gives. Runs then
  differ from the file-based version at about 1e-15. `ROSY_PS_ORDERED_SUM`
  adds in particle-number order for exact comparisons; it gathers
  everything and is not for production.
- **Empty ranks.** PHAD sets its series cutoff (`DAEPS`) only inside the
  near-field integrator. A rank that owns no leaves in a step never runs
  it and would do the next pass with the default cutoff. The port sets the
  cutoff after the near-field loop on every rank.
- **Sampling.** Range boundaries come from samples in proportion to the
  weight each rank holds. A fixed number per rank left half the ranks
  empty when all particles started on one rank.
- **Balance.** The first build has only an estimate (neighbourhood size
  squared over leaf size, plus a constant for the passes). After that the
  program reports what each leaf cost (`PSCOST`) and the store times its
  own passes; the next build balances by each particle's share of a
  step's work, averaged with earlier steps. Shares, not seconds, because
  PHAD has steps that cost twenty times more than the rest and have a
  different shape. In ordinary steps the slowest rank is within 10 to 30%
  of the average; in those heavy steps it is still about twice the
  average, since nothing predicts them.
- **Deterministic balance.** Measured times differ from run to run, so
  the particle split does too. Where results depend on the split (PHAD's
  integrate-once near field steps all of a rank's particles together),
  that makes runs differ at rounding level, more where PHAD is sensitive.
  With `ROSY_PS_DETERMINISTIC` set, the store balances by counted work
  only: the program reports counted near-field work through `PSCOST`, and
  the store counts each owned box's translations (M2M, M2L, L2L, and
  particles summed into multipoles) from its plan, at
  `ROSY_PS_TRANSLATION_SECONDS` each (default 2e-6), charged to the
  particles it holds in that box. The same inputs on the same number of
  ranks then give the same split and the same output files, every run.
- **Output.** Writing PHAD's output files on the first rank cost more
  than the near field once the kernels were native: every `WRITE` was its
  own system call, and the other ranks waited. Each rank now formats its
  own run of particles (`PSSLICE`) and `PSWRITE` gathers the lines to the
  first rank, which writes each file once. Taking turns to append was
  slower still: PHAD's `BARRIER` is a whole `PLOOP` gather, and each turn
  reopened the file. File writes are also buffered now (see
  `rosy_lib::core::file_io`), flushed whenever another unit, process or
  program could look at the file.
- **One partition, several phases.** The passes and the near field are
  separated by barriers, so each would need its own balance. One set of
  ranges can only balance their sum.
- **Neighbour order.** `PSLISTS` returns neighbours box by box, as fmmcpp
  wrote them; the port sorts each leaf's list with PHAD's own `QSORT`.

## The near-field kernel

`COULSUM y mq i n wx wy wz want phi radii` replaces the pair loop of
PHAD's near-field integrator with one call. It adds up
`n_k (r_i - r_k) / |r_i - r_k|^3` over the other particles, each
coordinate a series in time, on plain coefficient arrays. It is not a
store call and needs no MPI.

The distance is measured in the rest frame of the beam's common velocity
`W = gamma*beta`, as PHAD's Nov 2026 version does it (`SC_D2`):
`DOT := WX*DX + WY*DY + WZ*DZ`, `R2 := DX*DX + DY*DY + DZ*DZ + DOT*DOT`.
(This replaces `COULSUM y mq i n relg ...`, which used the Oct 2026 form
`DX*DX + DY*DY + RELG^2*DZ*DZ` for a beam moving along z.) The force
scaling after the loop (`SC_G`) stays in the program.

- **Same bits.** Every coefficient comes from the same operations in the
  same order as the series operators use, cutoff included. PHAD's output
  files are unchanged.
- **Each coefficient once.** A Picard pass raises the order by one, and
  coefficient `c` depends only on coefficients up to `c` of the two
  particles. The kernel keeps what earlier passes worked out and checks
  its inputs bit for bit on every call, so nothing stale is reused.
- **One exception.** The operators add a product's terms in the order of
  the left operand's term list. A series with a nonzero coefficient right
  after a zero one gets its list out of order, and the rounding can then
  differ. The kernel detects this and runs the operators for that call.
- **Layout.** Particle `p` has its six series at `6(p-1)+1 ..= 6(p-1)+6`
  (positions, then momenta) and its charge factor at `mq(2p)`. The sums
  go to `phi(6(i-1)+4 ..= 6(i-1)+6)`. With `want` nonzero, `radii` gets
  one convergence-radius estimate per pair, exactly as PHAD's
  `RADIUS_FROM_SERIES` gives it for the squared distance (including
  `10^100` for a series whose ratios are all zero).
- **Checked against PHAD's code.** `examples/pstore/coulsum.fox` runs
  PHAD's own `SC_D2`, `RADIUS_FROM_SERIES` and pair loop next to
  `COULSUM` and compares every coefficient and radius.

PHAD's time stepping, error control and external fields stay in PHAD.

## Kernels a program supplies

`KERNEL 'file.c' 'function' a b c ...;` calls a C function that the program
ships with. This is how PHAD's near-field integrator and lattice run as
native code without that physics living in Rosy.

- The file is compiled and linked into the program. Its path is relative to
  the script; headers beside it are picked up.
- Each variable goes over as an array of doubles. A real is an array of one.
  Whatever the function changes is written back to the variable.
- The function has one fixed shape and must stay inside the lengths it is
  given:

  ```c
  void function(long long count, double **values, const long long *lengths);
  ```

- `ROSY_KERNEL_CFLAGS` and `ROSY_KERNEL_LIBS`, read when the program is
  built, add compiler flags and libraries to link.
- Arguments are copied in and out, so call it once for a whole batch (every
  leaf a process owns), not once per particle.
- It needs the `mpi` build for now, because the argument conversion lives
  with the store.

An optional third string gives the kind of each argument, one letter per
argument (missing letters mean `r`):

- `r`: as above, an array of doubles.
- `d`: an array of untyped cells holding DA values (expansions). Each cell
  goes over as a block of `1 + M` doubles, `M` the number of DA monomials:
  a flag (1 when the cell holds a DA), then the coefficients ordered by
  total degree, then by exponents with higher powers of earlier variables
  first (degree 2 in three variables: x1², x1x2, x1x3, x2², x2x3, x3²).
  A block left with flag 1 becomes a DA, coefficients at or below the DA
  epsilon dropped; a block left with flag 0 leaves its cell alone. The
  order is fixed by this interface, not by how Rosy stores DA values.

```
KERNEL 'fmm_rosy.c' 'phad_fmm_down' 'rrdd' PAR W MULTEXP LOCEXP;
```

`examples/kernel.fox` and `examples/kernel_da.fox` are small checks.

## Decisions

- **Where the tree code lives.** Built into `rosy-lib` as intrinsics.
- **fmmcpp's exact tree is kept**, so results stay comparable box for box.
- **Output order.** Sort by `id` on output steps, so output files keep
  today's format and order.

## Language surface (decided: keep PHAD's array style)

Intrinsics fill ordinary local arrays, so loop bodies stay as written.
These are implemented (`examples/pstore.fox` exercises them). Every argument
is a variable, and every rank makes the same call:

| Intrinsic | Purpose |
|---|---|
| `PSLOAD x y z px py pz f n e count` | Hand this rank's input particles to the store. Particle numbers continue from the last load, lower ranks first. |
| `PSBUILD q wx wy wz relg` | Recompute keys, migrate, rebuild leaves, exchange the halo. Keys come from positions boosted into the beam's rest frame exactly as PHAD's `SC_A` does it (`DOT := (WX*X+WY*Y+WZ*Z)/(RELG+1)`, `AX := X+WX*DOT`), with `W = gamma*beta` and `relg` as the program computed them. Stored positions stay in the lab frame. (Replaces `PSBUILD q zscale`, which scaled z by relg for the Oct 2026 PHAD, whose beam moved along z.) |
| `PSGET x y z px py pz f n e ids nown` | Fill local arrays: owned particles first, then halo copies. `ids` are particle numbers from 1. |
| `PSLISTS snumbs start_snumbs nnumbs start_nnumbs cx cy cz nleaves` | The per-leaf source and neighbour lists that `FMMDIRECT` reads from `file_4` today, as local positions. |
| `PSPUT x y z px py pz e` | Write owned results back to the store. |
| `PSSCALED sx sy sz` | Positions in the unit cube the tree was built in: boosted, then scaled. |
| `PSSIZES nmult nloc depth` | Multipoles and local expansions this rank holds, copies included, and the level count. |
| `PSLEAFLOC index` | Position of each leaf's local expansion, in `PSLISTS` order. |
| `PSUP level list count` | Upward-pass work list for one level. |
| `PSDOWN level list count` | Downward-pass work list for one level. |
| `PSMSYNC cells level` / `PSLSYNC cells level` | Exchange one level's multipoles or local expansions between ranks. |
| `PSHALO` | Refresh halo copies after `PSPUT`. |
| `PSCOST leaf seconds` | Charge measured work to an owned leaf. The next `PSBUILD` balances ranks by these costs plus each rank's own pass time. |
| `PSBOUNDS xmin ymin zmin dzero` | The cube of the last `PSBUILD`, in the rest frame. |
| `PSPREV ids start` | For each owned particle, the particle numbers around it before the last `PSBUILD`. |
| `PSPOS ids positions` | Local position of each particle number, 0 if not held here. |
| `PSSUM values count total` | Sum over all ranks, correctly rounded. |
| `PSMAX values count max` | The largest of the first `count` values over all ranks, on every rank. Exact, so independent of the rank count. |
| `PSALL x y z px py pz f n e count` | Every particle in number order on the first rank, for output. |
| `PSSLICE x y z px py pz f n e first count` | This rank's share of the particles in number order: numbers `first .. first+count-1`, rank r getting the r-th of equal runs. Each rank formats its own part of an output file. |
| `PSWRITE unit lines count` | The first `count` strings of every rank's `lines`, lower ranks first, written as lines to file `unit` by the first rank in one go (only it needs the file open). With `PSSLICE`, output is formatted in parallel and comes out in particle order. |

## Open decisions


- **Language surface.** How `phad.fox` declares distributed particles and
  loops over owned leaves. The halo design keeps loop bodies as they are, but
  the declarations and the `PLOOP`/gather pairs must change.
