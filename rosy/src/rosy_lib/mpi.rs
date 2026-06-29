use anyhow::{Context, Result, bail, ensure};
use bincode::{Decode, Encode, config::Configuration};
use mpi::{environment::Universe, topology::SimpleCommunicator, traits::*};

use crate::rosy_lib::RE;

pub struct RosyMPIContext {
    pub universe: Universe,
    pub world: SimpleCommunicator,
    pub size: i32,
    pub rank: i32,
    pub bincode_config: Configuration,
}
impl RosyMPIContext {
    pub fn new() -> Result<Self> {
        let universe = mpi::initialize().context("Failed to initialize MPI")?;
        let world = universe.world();
        Ok(RosyMPIContext {
            universe,
            size: world.size(),
            rank: world.rank(),
            world,
            bincode_config: bincode::config::standard(),
        })
    }
    // Coordinates a value array between all different processes
    //  according to the specified communication standard.
    pub fn coordinate<T: Encode + Decode<()> + std::fmt::Debug + Default>(
        &self,

        value: &mut Vec<T>,
        communication_standard: u8,
        num_groups: &mut RE,
    ) -> Result<()> {
        match communication_standard {
            1 => {
                // In this standard, each process sends to all other processes
                //  in its group.
                //
                // For example, for 6 processes and 3 groups,
                // - Process 0 sends/recieves from processes 2 and 4
                // - Process 1 sends/recieves from processes 3 and 5
                // - Process 2 sends/recieves from processes 0 and 4
                // - Process 3 sends/recieves from processes 1 and 5
                // - Process 4 sends/recieves from processes 0 and 2
                // - Process 5 sends/recieves from processes 1 and 3
                let group_num = self
                    .get_group_num(num_groups)
                    .context("Failed to get group number")?;
                let num_groups = *num_groups as i32;
                let processes_per_group = self.size / num_groups;
                let group_id = self.rank % processes_per_group;

                // The output array must have at least `num_groups` slots —
                // one per group's contribution. The user's array may have
                // been sized from a variable (e.g. `(RE NP) X`) before
                // `PNPRO NP` ran, in which case it's still empty here.
                if value.len() < num_groups as usize {
                    value.resize_with(num_groups as usize, T::default);
                }

                let other_nodes: Vec<i32> = (0..self.size)
                    .filter(|r| (r % processes_per_group) == group_id && *r != self.rank)
                    .collect();

                // Get the value we're going to be sending,
                //  which is the group_num'th element of the array
                let binary_value: Vec<u8> =
                    bincode::encode_to_vec(&value[group_num as usize], self.bincode_config)
                        .context("Failed to serialize value for communication")?;

                // Send this value to all other nodes in the group
                for to_send in other_nodes.iter() {
                    self.world.process_at_rank(*to_send).send(&binary_value);
                }

                // Now receive values from all other nodes in the group
                for _ in other_nodes.iter() {
                    let (msg, status) = self.world.any_process().receive_vec::<u8>();
                    let recieved_from = status.source_rank() as usize;
                    let (decoded_value, _): (T, _) =
                        bincode::decode_from_slice(&msg, self.bincode_config)
                            .context("Failed to deserialize received value")?;

                    // Store the received value in the appropriate position
                    let recieved_from_group = (recieved_from as i32) / processes_per_group;
                    value[recieved_from_group as usize] = decoded_value;
                }

                Ok(())
            }
            _ => bail!(
                "Unsupported communication standard: {}",
                communication_standard
            ),
        }
    }
    pub fn get_group_num(&self, num_groups: &mut RE) -> Result<RE> {
        let num_groups = *num_groups as i32;
        let processes_per_group = self.size / num_groups;

        ensure!(
            self.size % num_groups == 0,
            "Total number of processes ({}) must be divisible by the number of processes per group ({})!",
            self.size,
            num_groups
        );
        let group_num = self.rank / processes_per_group;

        Ok(group_num as RE)
    }
    fn _get_root_rank(&self, num_groups: &mut RE) -> Result<RE> {
        let num_groups = *num_groups as i32;

        ensure!(
            self.size % num_groups == 0,
            "Total number of processes ({}) must be divisible by the number of processes per group ({})!",
            self.size,
            num_groups
        );
        let processes_per_group = self.size / num_groups;
        let root_rank = self.rank - (self.rank % processes_per_group);

        Ok(root_rank as RE)
    }
}
