#![allow(unused_variables)]
#![allow(unused_assignments)]
#![allow(unused_imports)]
#![allow(unused_mut)]
#![allow(unused_parens)]
#![allow(dead_code)]
#![allow(non_snake_case)]

use rosy_lib::*;
use anyhow::{Result, Context, ensure, bail};
use num_complex::Complex64;
use std::sync::OnceLock;
use std::time::Instant;

static ROSY_PROGRAM_START: OnceLock<Instant> = OnceLock::new();

fn rosy_init_timer() {
	let _ = ROSY_PROGRAM_START.set(Instant::now());
}

fn rosy_elapsed_seconds() -> f64 {
	ROSY_PROGRAM_START
		.get_or_init(Instant::now)
		.elapsed()
		.as_secs_f64()
}

fn main_wrapper() -> Result<()> {
	rosy_init_timer();
	let start = std::time::Instant::now();
	// <MPI_START>
	let mut rosy_mpi_context_inner = RosyMPIContext::new()
		.context("Failed to initialize Rosy MPI context")?;
	let rosy_mpi_context: &mut RosyMPIContext = &mut rosy_mpi_context_inner;
	let group_num = rosy_mpi_context.get_group_num(&mut  &mut 1.0f64)
		.context("Failed to get group number")? + 1.0f64;
	// <MPI_END>

	// Initialize Taylor series system (lightweight default — DAINI/OV overrides this)
	taylor::init_taylor(3, 6)
		.context("Failed to initialize Taylor system")?;

    // <INJECT_START>
	let mut X: f64 = 0.0;
	let mut Y: f64 = 0.0;
	X = (&mut 2f64).to_owned();
	Y = (&mut -3f64).to_owned();
	println!("{}", &mut RosyST::rosy_to_string(&*&mut RosyAdd::rosy_add(&*&mut X, &*&mut Y)));
	// <INJECT_END>
    
    Ok(())
}
fn main() -> Result<()> {
	if let Err(err) = main_wrapper() {
		let mut err_str = format!("{}", err.root_cause());

        for (ind, ctx) in err.chain().rev().enumerate().skip(1) {
            if let Some(src) = ctx.source() {
				let src = format!("{}", src);
				let src = if src.len() > 1000 && std::env::var("RUST_BACKTRACE").unwrap_or_default() != "1" {
					format!("{}... (truncated)", &src[..1000])
				} else {
					src
				};

				err_str += &format!("\n {ind}: {src}");
			}
        }

		// Check if the user has backtraces enabled
		if std::env::var("RUST_BACKTRACE").unwrap_or_default() == "1" {
			err_str += "\n\nFull backtrace...";
			err_str += &format!("{:#?}", err.backtrace());
		} else {
			err_str += "\n\nSet RUST_BACKTRACE=1 for a complete backtrace.";
		}

		bail!(err_str);
	}

	Ok(())
}