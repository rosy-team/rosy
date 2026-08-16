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
//! - **[`scrlen`]** — `SCRLEN c;` — scratch memory (no-op, COSY compat)
//! - **[`pnpro`]** — `PNPRO var;` — number of concurrent processes

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
pub mod procedure;
pub mod procedure_call;
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
