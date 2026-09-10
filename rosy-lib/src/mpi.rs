use mpi::{traits::*, topology::SimpleCommunicator, environment::Universe};
use bincode::{Encode, Decode, config::Configuration};
use anyhow::{Result, Context, ensure, bail};

use crate::{CD, DA, RosyValue, RE};
use num_complex::Complex64;
use bincode::enc::Encoder;
use bincode::de::Decoder;
use bincode::error::{DecodeError, EncodeError};


pub struct RosyMPIContext {
	pub universe: Universe,
	pub world: SimpleCommunicator,
	pub size: i32,
	pub rank: i32,
	pub bincode_config: Configuration,
}
impl RosyMPIContext {
    pub fn new () -> Result<Self> {
        let universe = mpi::initialize()
            .context("Failed to initialize MPI")?;
        let world = universe.world();
        Ok(RosyMPIContext {
            universe,
            size: world.size(),
            rank: world.rank(),
            world,
            bincode_config: bincode::config::standard()
        })
    }
    // Coordinates a value array between all different processes
    //  according to the specified communication standard.
    pub fn coordinate<T: Encode + Decode<()> + std::fmt::Debug + Default> (
        &self,

        value: &mut Vec<T>,
        communication_standard: u8,
        num_groups: &mut RE
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
                let group_num = self.get_group_num(num_groups)
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
                let binary_value: Vec<u8> = bincode::encode_to_vec(&value[group_num as usize], self.bincode_config)
                    .context("Failed to serialize value for communication")?;

                // Send this value to all other nodes in the group
                for to_send in other_nodes.iter() {
                    self.world.process_at_rank(*to_send)
                        .send(&binary_value);
                }

                // Now receive values from all other nodes in the group
                for _ in other_nodes.iter() {
                    let (msg, status) = self.world.any_process().receive_vec::<u8>();
                    let recieved_from = status.source_rank() as usize;
                    let (decoded_value, _): (T, _) = bincode::decode_from_slice(&msg, self.bincode_config)
                        .context("Failed to deserialize received value")?;
                    
                    // Store the received value in the appropriate position
                    let recieved_from_group = (recieved_from as i32) / processes_per_group;
                    value[recieved_from_group as usize] = decoded_value;
                }

                Ok(())
            },
            _ => bail!( "Unsupported communication standard: {}", communication_standard )
        }
    }
    pub fn get_group_num ( 
        &self,
        num_groups: &mut RE
    ) -> Result<RE> {
        let num_groups = *num_groups as i32;
        let processes_per_group = self.size / num_groups;

        ensure!(self.size % num_groups == 0, "Total number of processes ({}) must be divisible by the number of processes per group ({})!", self.size, num_groups);
        let group_num = self.rank / processes_per_group;

        Ok(group_num as RE)
    }
    fn _get_root_rank ( 
        &self,
        num_groups: &mut RE,
    ) -> Result<RE> {
        let num_groups = *num_groups as i32;

        ensure!(self.size % num_groups == 0, "Total number of processes ({}) must be divisible by the number of processes per group ({})!", self.size, num_groups);
        let processes_per_group = self.size / num_groups;
        let root_rank = self.rank - (self.rank % processes_per_group);

        Ok(root_rank as RE)
    }
}

impl Encode for DA {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        self.coeffs.encode(encoder)?;
        self.nonzero.encode(encoder)
    }
}

impl<C> Decode<C> for DA {
    fn decode<D: Decoder<Context = C>>(decoder: &mut D) -> Result<Self, DecodeError> {
        Ok(Self {
            coeffs: Decode::decode(decoder)?,
            nonzero: Decode::decode(decoder)?,
        })
    }
}

impl Encode for CD {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        self.nonzero.encode(encoder)?;
        (self.coeffs.len() as u64).encode(encoder)?;
        for c in &self.coeffs {
            c.re.encode(encoder)?;
            c.im.encode(encoder)?;
        }
        Ok(())
    }
}

impl<C> Decode<C> for CD {
    fn decode<D: Decoder<Context = C>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let nonzero: Vec<u32> = Decode::decode(decoder)?;
        let n = u64::decode(decoder)? as usize;
        let mut coeffs = Vec::with_capacity(n);
        for _ in 0..n {
            let re = f64::decode(decoder)?;
            let im = f64::decode(decoder)?;
            coeffs.push(Complex64::new(re, im));
        }
        Ok(Self { coeffs, nonzero })
    }
}

const RV_RE: u8 = 0;
const RV_ST: u8 = 1;
const RV_LO: u8 = 2;
const RV_CM: u8 = 3;
const RV_VE: u8 = 4;
const RV_DA: u8 = 5;
const RV_CD: u8 = 6;
const RV_ARR: u8 = 7;

impl Encode for RosyValue {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        match self {
            RosyValue::RE(v) => {
                RV_RE.encode(encoder)?;
                v.encode(encoder)
            }
            RosyValue::ST(v) => {
                RV_ST.encode(encoder)?;
                v.encode(encoder)
            }
            RosyValue::LO(v) => {
                RV_LO.encode(encoder)?;
                v.encode(encoder)
            }
            RosyValue::CM(v) => {
                RV_CM.encode(encoder)?;
                v.re.encode(encoder)?;
                v.im.encode(encoder)
            }
            RosyValue::VE(v) => {
                RV_VE.encode(encoder)?;
                v.encode(encoder)
            }
            RosyValue::DA(v) => {
                RV_DA.encode(encoder)?;
                v.encode(encoder)
            }
            RosyValue::CD(v) => {
                RV_CD.encode(encoder)?;
                v.encode(encoder)
            }
            RosyValue::Arr(v) => {
                RV_ARR.encode(encoder)?;
                v.encode(encoder)
            }
        }
    }
}

impl<C> Decode<C> for RosyValue {
    fn decode<D: Decoder<Context = C>>(decoder: &mut D) -> Result<Self, DecodeError> {
        match u8::decode(decoder)? {
            RV_RE => Ok(RosyValue::RE(Decode::decode(decoder)?)),
            RV_ST => Ok(RosyValue::ST(Decode::decode(decoder)?)),
            RV_LO => Ok(RosyValue::LO(Decode::decode(decoder)?)),
            RV_CM => {
                let re = f64::decode(decoder)?;
                let im = f64::decode(decoder)?;
                Ok(RosyValue::CM(Complex64::new(re, im)))
            }
            RV_VE => Ok(RosyValue::VE(Decode::decode(decoder)?)),
            RV_DA => Ok(RosyValue::DA(Decode::decode(decoder)?)),
            RV_CD => Ok(RosyValue::CD(Decode::decode(decoder)?)),
            RV_ARR => Ok(RosyValue::Arr(Decode::decode(decoder)?)),
            _ => Err(DecodeError::Other("unknown RosyValue tag")),
        }
    }
}