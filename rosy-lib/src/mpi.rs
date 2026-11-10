use anyhow::{Context, Result, bail, ensure};
use bincode::{Decode, Encode, config::Configuration};
use mpi::{environment::Universe, topology::SimpleCommunicator, traits::*};

use crate::{CD, DA, RE, RosyValue};
use bincode::de::Decoder;
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use num_complex::Complex64;

pub struct RosyMPIContext {
    pub universe: Universe,
    pub world: SimpleCommunicator,
    pub size: i32,
    pub rank: i32,
    pub bincode_config: Configuration,
    /// Tag for the next `coordinate` call. Every rank increments in lockstep.
    /// Untagged receives were taking the next row's message.
    coord_tag: i32,
    /// Group count of each PLOOP this rank is inside, outermost first.
    ploop_groups: Vec<i32>,
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
            coord_tag: 1,
            ploop_groups: Vec::new(),
        })
    }

    /// Enter a PLOOP that splits the current group into `num_groups`.
    pub fn ploop_enter(&mut self, num_groups: RE) {
        self.ploop_groups.push(num_groups as i32);
    }

    pub fn ploop_exit(&mut self) {
        self.ploop_groups.pop();
    }

    /// COSY `PROOT`: 1 on the root of the current group, 0 elsewhere.
    /// Outside any PLOOP the group is the whole world. Inside `PLOOP I 1 NP`
    /// every group is one rank, so every rank is a root.
    pub fn proot(&self) -> RE {
        let mut tasks = self.size;
        let mut local = self.rank;
        for &groups in &self.ploop_groups {
            tasks /= groups.max(1);
            if tasks > 0 {
                local %= tasks;
            }
        }
        if local == 0 { 1.0 } else { 0.0 }
    }

    /// MPI tag for this gather. Wraps inside the required minimum of 32767.
    fn next_tag(&mut self) -> mpi::Tag {
        let tag = self.coord_tag;
        self.coord_tag = if tag >= 32767 { 1 } else { tag + 1 };
        tag
    }
    // Coordinates a value array between all different processes
    //  according to the specified communication standard.
    pub fn coordinate<T: Encode + Decode<()> + std::fmt::Debug + Default>(
        &mut self,
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

                // `PLOOP I 1 NP` with a 1-D output (PHAD's BARRIER, status and
                // timer arrays): every rank is its own group, so one allgather
                // replaces P-1 sends and P-1 wildcard receives per rank.
                if processes_per_group == 1 {
                    let (recv, counts, displs) = self.allgather_bytes(&binary_value)?;
                    for r in 0..self.size as usize {
                        if r == self.rank as usize {
                            continue;
                        }
                        let start = displs[r] as usize;
                        let end = start + counts[r] as usize;
                        let (decoded, _): (T, _) =
                            bincode::decode_from_slice(&recv[start..end], self.bincode_config)
                                .context("Failed to deserialize received value")?;
                        value[r] = decoded;
                    }
                    return Ok(());
                }

                // One tag per call. The next row must not satisfy this receive.
                // Standard send: a synchronous send here deadlocks, because
                // every rank sends before anyone posts the matching receive.
                let tag = self.next_tag();
                for to_send in other_nodes.iter() {
                    self.world
                        .process_at_rank(*to_send)
                        .send_with_tag(&binary_value, tag);
                }

                for _ in other_nodes.iter() {
                    let (msg, status) = self.world.any_process().receive_vec_with_tag::<u8>(tag);
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

    /// One exchange for every rank-axis row of a 2D PLOOP output.
    ///
    /// The per-cell `coordinate` loop was one all-to-all per scalar. PHAD's
    /// long-range gather is about 60 by 1000 of those, and each one probes
    /// every other rank. A step was spending the hour inside that loop.
    pub fn coordinate_rows<T>(
        &mut self,
        rows: &mut Vec<Vec<T>>,
        communication_standard: u8,
        num_groups: &mut RE,
    ) -> Result<()>
    where
        T: Encode + Decode<()> + std::fmt::Debug + Default + Clone,
    {
        ensure!(
            communication_standard == 1,
            "Unsupported communication standard: {}",
            communication_standard
        );
        let n = self.sync_extent(rows.len())?;
        rows.resize_with(n, Vec::new);
        let local_max = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        let width = self.sync_extent(local_max)?;
        let need = width.max(*num_groups as usize);
        for row in rows.iter_mut() {
            if row.len() < need {
                row.resize_with(need, T::default);
            }
        }
        let (group_num, num_groups, partners) = self.group_partners(num_groups)?;
        // A few megabytes per message. One DA column stays under the 2GB MPI cap.
        const CHUNK: usize = 4096;
        for chunk in rows.chunks_mut(CHUNK) {
            self.exchange_chunk(chunk, group_num, num_groups, &partners)?;
        }
        Ok(())
    }

    /// One exchange per plane of a 3D PLOOP output (`q` by leaves by rank).
    pub fn coordinate_grid<T>(
        &mut self,
        grid: &mut Vec<Vec<Vec<T>>>,
        communication_standard: u8,
        num_groups: &mut RE,
    ) -> Result<()>
    where
        T: Encode + Decode<()> + std::fmt::Debug + Default + Clone,
    {
        let n0 = self.sync_extent(grid.len())?;
        grid.resize_with(n0, Vec::new);
        let n1_local = grid.iter().map(|plane| plane.len()).max().unwrap_or(0);
        let n1 = self.sync_extent(n1_local)?;
        for plane in grid.iter_mut() {
            plane.resize_with(n1, Vec::new);
        }
        for plane in grid.iter_mut() {
            self.coordinate_rows(plane, communication_standard, num_groups)?;
        }
        Ok(())
    }

    fn group_partners(&self, num_groups: &mut RE) -> Result<(i32, i32, Vec<i32>)> {
        let group_num = self.get_group_num(num_groups)? as i32;
        let num_groups = *num_groups as i32;
        let processes_per_group = self.size / num_groups;
        let group_id = self.rank % processes_per_group;
        let partners: Vec<i32> = (0..self.size)
            .filter(|r| (r % processes_per_group) == group_id && *r != self.rank)
            .collect();
        Ok((group_num, num_groups, partners))
    }

    /// Every rank's `payload`, concatenated in rank order, with per-rank
    /// byte counts and offsets. One count allgather plus one `MPI_Allgatherv`.
    fn allgather_bytes(
        &mut self,
        payload: &[u8],
    ) -> Result<(Vec<u8>, Vec<mpi::Count>, Vec<mpi::Count>)> {
        use mpi::datatype::PartitionMut;
        let size = self.size as usize;
        let mine = mpi::Count::try_from(payload.len())
            .context("PLOOP gather slab is over 2GB; it must be split")?;
        let mut counts = vec![0 as mpi::Count; size];
        self.world.all_gather_into(&mine, &mut counts[..]);
        let mut displs = vec![0 as mpi::Count; size];
        let mut total: i64 = 0;
        for r in 0..size {
            displs[r] = mpi::Count::try_from(total)
                .context("PLOOP gather is over 2GB in total; it must be split")?;
            total += counts[r] as i64;
        }
        let mut recv = vec![0u8; total as usize];
        {
            let mut partition = PartitionMut::new(&mut recv[..], &counts[..], &displs[..]);
            self.world.all_gather_varcount_into(payload, &mut partition);
        }
        Ok((recv, counts, displs))
    }

    /// One `MPI_Allgatherv` of each rank's encoded slab. Rank `r` owns column `r`.
    fn allgather_chunk<T>(&mut self, rows: &mut [Vec<T>], payload: &[u8]) -> Result<()>
    where
        T: Encode + Decode<()> + std::fmt::Debug + Clone,
    {
        let size = self.size as usize;
        let (recv, counts, displs) = self.allgather_bytes(payload)?;
        for r in 0..size {
            if r == self.rank as usize {
                continue;
            }
            let start = displs[r] as usize;
            let end = start + counts[r] as usize;
            let (decoded, _): (Vec<T>, _) =
                bincode::decode_from_slice(&recv[start..end], self.bincode_config)
                    .context("Failed to deserialize gather")?;
            ensure!(
                decoded.len() == rows.len(),
                "gather row count {} != {}",
                decoded.len(),
                rows.len()
            );
            for (row, value) in rows.iter_mut().zip(decoded) {
                row[r] = value;
            }
        }
        Ok(())
    }

    fn exchange_chunk<T>(
        &mut self,
        rows: &mut [Vec<T>],
        group_num: i32,
        num_groups: i32,
        partners: &[i32],
    ) -> Result<()>
    where
        T: Encode + Decode<()> + std::fmt::Debug + Clone,
    {
        let tag = self.next_tag();
        let owned: Vec<T> = rows
            .iter()
            .map(|row| row[group_num as usize].clone())
            .collect();
        let payload = bincode::encode_to_vec(&owned, self.bincode_config)
            .context("Failed to serialize gather")?;
        let processes_per_group = self.size / num_groups;
        // `PLOOP I 1 NP`: every rank is its own group, so this is one
        // allgather over the world, as in COSY. The pairwise loop below was
        // a chain of P*(P-1)/2 rendezvous per chunk.
        if processes_per_group == 1 {
            return self.allgather_chunk(rows, &payload);
        }
        // Partners are in ascending rank order. One pair at a time, and the
        // lower rank sends first. Sending to every peer before any receive
        // deadlocks once a slab is bigger than the MPI eager buffer.
        for &partner in partners {
            let msg = if self.rank < partner {
                self.world
                    .process_at_rank(partner)
                    .send_with_tag(&payload, tag);
                self.world
                    .process_at_rank(partner)
                    .receive_vec_with_tag::<u8>(tag)
                    .0
            } else {
                let msg = self
                    .world
                    .process_at_rank(partner)
                    .receive_vec_with_tag::<u8>(tag)
                    .0;
                self.world
                    .process_at_rank(partner)
                    .send_with_tag(&payload, tag);
                msg
            };
            let from_group = partner / processes_per_group;
            let (decoded, _): (Vec<T>, _) = bincode::decode_from_slice(&msg, self.bincode_config)
                .context("Failed to deserialize gather")?;
            ensure!(
                decoded.len() == rows.len(),
                "gather row count {} != {}",
                decoded.len(),
                rows.len()
            );
            for (row, value) in rows.iter_mut().zip(decoded) {
                row[from_group as usize] = value;
            }
        }
        Ok(())
    }
    /// Every rank must agree on how many `coordinate` calls a PLOOP runs.
    /// COSY arrays are a fixed rectangle. A Rosy `Vec` only grows to the
    /// highest index that rank wrote, so a `for i in 0..value.len()` gather
    /// deadlocks when the lengths differ. Returns the max length.
    pub fn sync_extent(&self, len: usize) -> Result<usize> {
        use mpi::collective::{CommunicatorCollectives, SystemOperation};
        let local = len as i32;
        let mut global = 0i32;
        self.world
            .all_reduce_into(&local, &mut global, SystemOperation::max());
        Ok(global.max(0) as usize)
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
