//! # Core Statements
//!
//! Declarations, control flow, functions, procedures, and utility operations.
//!
//! ## Declarations & Assignment
//!
//! - **[`var_decl`]** — `VARIABLE (type) name;`
//! - **[`assign`]** — `name := expr;`
//!
//! ## Control Flow
//!
//! - **[`loop`]** — `LOOP i start end [step]; ... ENDLOOP;`
//! - **[`while_loop`]** — `WHILE cond; ... ENDWHILE;`
//! - **[`ploop`]** — `PLOOP ... ENDPLOOP;` (MPI parallel)
//! - **[`if`]** — `IF cond; ... [ELSEIF ...;] [ELSE;] ENDIF;`
//! - **[`break`]** — `BREAK;`
//! - **[`quit`]** — `QUIT;`
//!
//! ## Functions & Procedures
//!
//! - **[`function`]** / **[`function_call`]** — define and call functions
//! - **[`procedure`]** / **[`procedure_call`]** — define and call procedures
//!
//! ## String & Conversion
//!
//! - **[`substr`]** — `SUBSTR source first last dest;` — extract substring
//! - **[`stcre`]** — `STCRE string result;` — parse string to real
//! - **[`recst`]** — `RECST value format result;` — format number as string
//!
//! ## Vector & Data
//!
//! - **[`velset`]** — `VELSET vec comp value;` — set vector component
//! - **[`reran`]** — `RERAN var;` — random number in [-1, 1]
//! - **[`ranseed`]** — `RANSEED seed;` — set global RNG seed
//! - **[`imunit`]** — `IMUNIT var;` — imaginary unit *i* as CM
//!
//! ## System
//!
//! - **[`scrlen`]** — `SCRLEN c;` — DA scratch size (set, or query if `c < 0`)
//! - **[`pnpro`]** — `PNPRO var;` — number of concurrent processes
//! - **[`proot`]** — `PROOT var;` — 1 on the root of the current PLOOP group, else 0
//!
//! ## Distributed Particle Store (MPI)
//!
//! All in **[`pstore`]**; every rank makes the same call. See
//! `docs/distributed-storage.md` for the design.
//!
//! - `PSLOAD x y z px py pz f n e count;` — hand this rank's particles to the store
//! - `PSBUILD q wx wy wz relg;` — rebuild the tree (in the beam's rest frame), migrate, exchange the halo
//! - `PSGET x y z px py pz f n e ids nown;` — owned particles, then halo copies
//! - `PSPUT x y z px py pz e;` — write owned particles back
//! - `PSHALO;` — refresh halo copies after `PSPUT`
//! - `PSLISTS snumbs start_snumbs nnumbs start_nnumbs cx cy cz nleaves;` — each leaf's own and neighbour particles, and its centre
//! - `PSLEAFLOC index;` — where each leaf's local expansion lives
//! - `PSPREV ids start;` — each particle's neighbours before the last build
//! - `PSPOS ids positions;` — local position of each particle number
//! - `PSBOUNDS xmin ymin zmin dzero;` — the cube of the last build
//! - `PSSCALED sx sy sz;` — positions in that cube
//! - `PSSIZES nmult nloc depth;` — expansions held and tree depth
//! - `PSUP level list count;` / `PSDOWN level list count;` — FMM work lists for one level
//! - `PSMSYNC cells level;` / `PSLSYNC cells level;` — exchange one level's multipoles / local expansions
//! - `PSSUM values count total;` — sum over all ranks, correctly rounded
//! - `PSMAX values count max;` — largest value over all ranks
//! - `PSALL x y z px py pz f n e count;` — every particle, in number order, on the first rank
//! - `PSSLICE x y z px py pz f n e first count;` — this rank's share, in number order
//! - `PSWRITE unit lines count;` — every rank's lines written to one file by the first rank
//! - `PSCOST leaf seconds;` — charge measured work to a leaf for the next build's balance
//! - `PSREMOVE keep info k removed nremoved ncool;` — drop particles, renumbering survivors
//!
//! Also in [`pstore`], though not store calls:
//!
//! - `EXTERN 'file' 'function' ['kinds'] args...;` — call a C or Rust function the program supplies
//! - `COULSUM y mq i n wx wy wz want phi radii;` — the Coulomb sum over Taylor series

pub mod argget;
pub mod assign;
pub mod r#break;
pub mod function;
pub mod function_call;
pub mod r#if;
pub mod imunit;
pub mod lfalse;
pub mod r#loop;
pub mod ltrue;
pub mod memall;
pub mod memdpv;
pub mod memfre;
pub mod memwrt;
pub mod ploop;
pub mod pnpro;
pub mod pstore;
pub mod procedure;
pub mod procedure_call;
pub mod proot;
pub mod quit;
pub mod ranseed;
pub mod recst;
pub mod reran;
pub mod scrlen;
pub mod sleepm;
pub mod stcre;
pub mod substr;
pub mod var_decl;
pub mod velset;
pub mod while_loop;
