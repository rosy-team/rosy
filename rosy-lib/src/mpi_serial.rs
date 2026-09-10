//! Serial stand-in for [`crate::mpi::RosyMPIContext`].
//!
//! `PNPRO` and procedure captures reference `rosy_mpi_context` even in
//! programs that never `PLOOP`. Those builds must not pull in the `mpi`
//! crate (libffi / system MPI). Size and rank are 1 and 0.

use anyhow::{ensure, Result};

use crate::RE;

pub struct RosyMPIContext {
    pub size: i32,
    pub rank: i32,
}

impl RosyMPIContext {
    pub fn new() -> Result<Self> {
        Ok(Self { size: 1, rank: 0 })
    }

    pub fn get_group_num(&self, num_groups: &mut RE) -> Result<RE> {
        let num_groups = *num_groups as i32;
        ensure!(
            num_groups != 0 && self.size % num_groups == 0,
            "Total number of processes ({}) must be divisible by the number of groups ({})!",
            self.size,
            num_groups
        );
        let processes_per_group = self.size / num_groups;
        Ok((self.rank / processes_per_group) as RE)
    }

    pub fn coordinate<T>(
        &self,
        _value: &mut Vec<T>,
        _communication_standard: u8,
        _num_groups: &mut RE,
    ) -> Result<()> {
        Ok(())
    }
}
