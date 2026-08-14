//! # I/O Statements
//!
//! File and console input/output. Unit `6` is standard output (console);
//! other unit numbers map to files opened with `OPENF`/`OPENFB`.
//!
//! - **[`mod@write`]** — `WRITE unit exprs;` — print formatted text
//! - **[`read`]** — `READ unit var;` — read a value
//! - **[`writeb`]** — `WRITEB unit exprs;` — write binary data
//! - **[`readb`]** — `READB unit var;` — read binary data
//! - **[`openf`]** — `OPENF unit file status;` — open a text file
//! - **[`openfb`]** — `OPENFB unit file status;` — open a binary file
//! - **[`closef`]** — `CLOSEF unit;` — close a file
//! - **[`cpusec`]** — `CPUSEC var;` — get CPU time
//! - **[`pwtime`]** — `PWTIME var;` — wall-clock elapsed time
//! - **[`os_call`]** — `OS cmd;` — execute a shell command
//! - **[`velget`]** — `VELGET unit var;` — read a vector from a file
//! - **[`save`]** — `SAVE name;` — save compiled code (no-op, COSY compat)

pub mod backf;
pub mod closef;
pub mod cpusec;
pub mod openf;
pub mod openfb;
pub mod os_call;
pub mod pwtime;
pub mod read;
pub mod readb;
pub mod readm;
pub mod reads;
pub mod rewf;
pub mod save;
pub mod velget;
pub mod write;
pub mod writeb;
pub mod writem;
