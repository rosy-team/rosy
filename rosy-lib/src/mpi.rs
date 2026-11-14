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
    /// Per-PLOOP timing, on when `ROSY_MPI_STATS` is set. See [`stats`].
    stats: Option<Box<stats::Stats>>,
    /// This rank's share of the distributed particles (`PS*` intrinsics).
    pub pstore: crate::pstore::ParticleStore,
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
            stats: stats::Stats::from_env(),
            pstore: Default::default(),
        })
    }

    /// Enter a PLOOP that splits the current group into `num_groups`.
    /// `line` is the source line of the PLOOP; it only labels the stats.
    pub fn ploop_enter(&mut self, num_groups: RE, line: u32) {
        // Another rank may read what this one wrote before the PLOOP.
        let _ = crate::core::file_io::rosy_flush_all_files();
        self.ploop_groups.push(num_groups as i32);
        if let Some(stats) = self.stats.as_mut() {
            stats.enter(line);
        }
    }

    pub fn ploop_exit(&mut self) {
        let _ = crate::core::file_io::rosy_flush_all_files();
        self.ploop_groups.pop();
        if let Some(stats) = self.stats.as_mut() {
            stats.exit();
        }
    }

    /// The PLOOP body is done and its gather is about to start. With stats
    /// on, a barrier first separates "waiting for the slowest rank" from
    /// the cost of the exchange itself.
    fn stats_gather_begin(&mut self) {
        if self.stats.as_ref().is_some_and(|s| s.needs_body_end()) {
            let t = std::time::Instant::now();
            self.world.barrier();
            if let Some(stats) = self.stats.as_mut() {
                stats.body_end(t);
            }
        }
    }

    fn stats_add(&mut self, f: impl FnOnce(&mut stats::Frame)) {
        if let Some(frame) = self.stats.as_mut().and_then(|s| s.top()) {
            f(frame);
        }
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
                self.stats_gather_begin();
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
                let t_encode = std::time::Instant::now();
                let binary_value: Vec<u8> =
                    bincode::encode_to_vec(&value[group_num as usize], self.bincode_config)
                        .context("Failed to serialize value for communication")?;
                let encode = t_encode.elapsed().as_secs_f64();
                let sent = binary_value.len() as u64;
                self.stats_add(|f| {
                    f.encode += encode;
                    f.bytes_out += sent;
                });

                // `PLOOP I 1 NP` with a 1-D output (PHAD's BARRIER, status and
                // timer arrays): every rank is its own group, so one allgather
                // replaces P-1 sends and P-1 wildcard receives per rank.
                if processes_per_group == 1 {
                    let t_wire = std::time::Instant::now();
                    let (recv, counts, displs) = self.allgather_bytes(&binary_value)?;
                    let wire = t_wire.elapsed().as_secs_f64();
                    let t_decode = std::time::Instant::now();
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
                    let decode = t_decode.elapsed().as_secs_f64();
                    let received = recv.len() as u64;
                    self.stats_add(|f| {
                        f.wire += wire;
                        f.decode += decode;
                        f.bytes_in += received;
                    });
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
        self.stats_gather_begin();
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
        self.stats_gather_begin();
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
        let t_wire = std::time::Instant::now();
        let (recv, counts, displs) = self.allgather_bytes(payload)?;
        let wire = t_wire.elapsed().as_secs_f64();
        let t_decode = std::time::Instant::now();
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
        let decode = t_decode.elapsed().as_secs_f64();
        let received = recv.len() as u64;
        self.stats_add(|f| {
            f.wire += wire;
            f.decode += decode;
            f.bytes_in += received;
        });
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
        let t_encode = std::time::Instant::now();
        let owned: Vec<T> = rows
            .iter()
            .map(|row| row[group_num as usize].clone())
            .collect();
        let payload = bincode::encode_to_vec(&owned, self.bincode_config)
            .context("Failed to serialize gather")?;
        let encode = t_encode.elapsed().as_secs_f64();
        let sent = payload.len() as u64;
        self.stats_add(|f| {
            f.encode += encode;
            f.bytes_out += sent;
        });
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

impl Drop for RosyMPIContext {
    /// Print the `ROSY_MPI_STATS` report. This runs before the fields drop,
    /// so MPI is still up for the one gather the report needs.
    fn drop(&mut self) {
        let Some(stats) = self.stats.take() else {
            return;
        };
        if std::thread::panicking() {
            return;
        }
        // Time inside each particle-store intrinsic, mean and max over ranks.
        let mut store_report = String::new();
        if !self.pstore.call_seconds.is_empty() {
            let mine = self.pstore.call_seconds.clone();
            let mut total = vec![0.0f64; mine.len()];
            let mut most = vec![0.0f64; mine.len()];
            self.world.all_reduce_into(&mine[..], &mut total[..], mpi::collective::SystemOperation::sum());
            self.world.all_reduce_into(&mine[..], &mut most[..], mpi::collective::SystemOperation::max());
            store_report.push_str("seconds inside each particle-store call (mean over ranks, [max]):\n");
            let names = crate::pstore::PS_INTRINSICS.iter().map(|(n, _)| *n).chain(crate::pstore::BUILD_PARTS.iter().copied());
            for (at, name) in names.enumerate() {
                if most[at] > 0.0005 {
                    store_report.push_str(&format!("  {name:<28} {:>10.3} {:>10.3}\n", total[at] / self.size as f64, most[at]));
                }
            }
        }
        let calls = crate::core::coulsum::COULSUM_CALLS.load(std::sync::atomic::Ordering::Relaxed);
        if calls > 0 {
            let slow = crate::core::coulsum::COULSUM_BY_OPERATORS.load(std::sync::atomic::Ordering::Relaxed);
            store_report.push_str(&format!("COULSUM on the first rank: {calls} calls, {slow} answered by the series operators\n"));
        }
        let payload = stats.encode();
        let bytes: Vec<u8> = payload.iter().flat_map(|v| v.to_le_bytes()).collect();
        let Ok((recv, counts, displs)) = self.allgather_bytes(&bytes) else {
            return;
        };
        if self.rank != 0 {
            return;
        }
        let per_rank: Vec<Vec<f64>> = (0..self.size as usize)
            .map(|r| {
                let start = displs[r] as usize;
                let end = start + counts[r] as usize;
                recv[start..end]
                    .chunks_exact(8)
                    .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
                    .collect()
            })
            .collect();
        let mut report = stats::report(&per_rank);
        report.push_str(&store_report);
        match stats.sink.as_deref() {
            Some(path) => {
                if let Err(e) = std::fs::write(path, &report) {
                    eprintln!("ROSY_MPI_STATS: cannot write {path}: {e}");
                    eprint!("{report}");
                }
            }
            None => eprint!("{report}"),
        }
    }
}

/// Opt-in timing of every PLOOP, for finding work that does not shrink as
/// ranks are added.
///
/// Set `ROSY_MPI_STATS=1` to print the report to stderr at exit, or to a
/// path to write it there. Each PLOOP site (keyed by source line) reports
/// its body time, the wait for the slowest rank, and the gather split into
/// encode, wire and decode. Serial code between two top-level PLOOPs is
/// reported per `previous line -> next line` pair. A body whose time stays
/// flat when the rank count doubles is replicated work, not parallel work.
mod stats {
    use std::collections::BTreeMap;
    use std::time::Instant;

    /// One PLOOP in progress.
    pub struct Frame {
        line: u32,
        enter: Instant,
        body_end: Option<Instant>,
        gather_start: Option<Instant>,
        pub encode: f64,
        pub wire: f64,
        pub decode: f64,
        pub bytes_out: u64,
        pub bytes_in: u64,
    }

    #[derive(Default, Clone, Copy)]
    struct Site {
        calls: f64,
        body: f64,
        wait: f64,
        gather: f64,
        encode: f64,
        wire: f64,
        decode: f64,
        bytes_out: f64,
        bytes_in: f64,
    }
    const SITE_FIELDS: usize = 9;

    pub struct Stats {
        start: Instant,
        sites: BTreeMap<u32, Site>,
        /// Serial time between top-level PLOOPs: `(previous, next)` lines.
        gaps: BTreeMap<(u32, u32), (f64, f64)>,
        stack: Vec<Frame>,
        last_exit: (u32, Instant),
        /// Report path. `None` prints to stderr.
        pub sink: Option<String>,
    }

    impl Stats {
        pub fn from_env() -> Option<Box<Stats>> {
            let value = std::env::var("ROSY_MPI_STATS").ok()?;
            let value = value.trim();
            if value.is_empty() || value == "0" {
                return None;
            }
            let sink = match value {
                "1" | "true" | "stderr" => None,
                path => Some(path.to_string()),
            };
            let now = Instant::now();
            Some(Box::new(Stats {
                start: now,
                sites: BTreeMap::new(),
                gaps: BTreeMap::new(),
                stack: Vec::new(),
                last_exit: (0, now),
                sink,
            }))
        }

        pub fn enter(&mut self, line: u32) {
            let now = Instant::now();
            if self.stack.is_empty() {
                let (prev, at) = self.last_exit;
                let gap = self.gaps.entry((prev, line)).or_default();
                gap.0 += 1.0;
                gap.1 += now.duration_since(at).as_secs_f64();
            }
            self.stack.push(Frame {
                line,
                enter: now,
                body_end: None,
                gather_start: None,
                encode: 0.0,
                wire: 0.0,
                decode: 0.0,
                bytes_out: 0,
                bytes_in: 0,
            });
        }

        pub fn top(&mut self) -> Option<&mut Frame> {
            self.stack.last_mut()
        }

        /// True until the innermost PLOOP's first gather call.
        pub fn needs_body_end(&self) -> bool {
            self.stack.last().is_some_and(|f| f.body_end.is_none())
        }

        /// `at` is when the body finished. Now is when every rank has.
        pub fn body_end(&mut self, at: Instant) {
            if let Some(frame) = self.stack.last_mut() {
                frame.body_end = Some(at);
                frame.gather_start = Some(Instant::now());
            }
        }

        pub fn exit(&mut self) {
            let now = Instant::now();
            let Some(frame) = self.stack.pop() else {
                return;
            };
            let body_end = frame.body_end.unwrap_or(now);
            let gather_start = frame.gather_start.unwrap_or(now);
            let site = self.sites.entry(frame.line).or_default();
            site.calls += 1.0;
            site.body += body_end.duration_since(frame.enter).as_secs_f64();
            site.wait += gather_start.duration_since(body_end).as_secs_f64();
            site.gather += now.duration_since(gather_start).as_secs_f64();
            site.encode += frame.encode;
            site.wire += frame.wire;
            site.decode += frame.decode;
            site.bytes_out += frame.bytes_out as f64;
            site.bytes_in += frame.bytes_in as f64;
            if self.stack.is_empty() {
                self.last_exit = (frame.line, now);
            }
        }

        /// Flat form for the report gather: wall, sites, then gaps.
        pub fn encode(&self) -> Vec<f64> {
            let mut out = vec![
                self.start.elapsed().as_secs_f64(),
                self.sites.len() as f64,
                self.gaps.len() as f64,
            ];
            for (line, s) in &self.sites {
                out.extend([
                    *line as f64,
                    s.calls,
                    s.body,
                    s.wait,
                    s.gather,
                    s.encode,
                    s.wire,
                    s.decode,
                    s.bytes_out,
                    s.bytes_in,
                ]);
            }
            for ((prev, next), (count, secs)) in &self.gaps {
                out.extend([*prev as f64, *next as f64, *count, *secs]);
            }
            out
        }
    }

    /// Mean, min and max of one quantity across ranks.
    #[derive(Default, Clone, Copy)]
    struct Spread {
        sum: f64,
        min: f64,
        max: f64,
        n: f64,
    }
    impl Spread {
        fn add(&mut self, v: f64) {
            if self.n == 0.0 {
                self.min = v;
                self.max = v;
            } else {
                self.min = self.min.min(v);
                self.max = self.max.max(v);
            }
            self.sum += v;
            self.n += 1.0;
        }
        fn mean(&self) -> f64 {
            if self.n == 0.0 { 0.0 } else { self.sum / self.n }
        }
    }

    pub fn report(per_rank: &[Vec<f64>]) -> String {
        use std::fmt::Write;
        let ranks = per_rank.len();
        let mut wall = Spread::default();
        let mut sites: BTreeMap<u32, [Spread; SITE_FIELDS]> = BTreeMap::new();
        let mut gaps: BTreeMap<(u32, u32), [Spread; 2]> = BTreeMap::new();
        for data in per_rank {
            if data.len() < 3 {
                continue;
            }
            wall.add(data[0]);
            let (n_sites, n_gaps) = (data[1] as usize, data[2] as usize);
            let mut at = 3;
            for _ in 0..n_sites {
                let Some(row) = data.get(at..at + 1 + SITE_FIELDS) else {
                    break;
                };
                let entry = sites.entry(row[0] as u32).or_default();
                for (spread, v) in entry.iter_mut().zip(&row[1..]) {
                    spread.add(*v);
                }
                at += 1 + SITE_FIELDS;
            }
            for _ in 0..n_gaps {
                let Some(row) = data.get(at..at + 4) else {
                    break;
                };
                let entry = gaps.entry((row[0] as u32, row[1] as u32)).or_default();
                entry[0].add(row[2]);
                entry[1].add(row[3]);
                at += 4;
            }
        }
        let mut out = String::new();
        let _ = writeln!(
            out,
            "[rosy mpi stats] ranks={ranks} wall={:.2}s (seconds are per-rank means; max over ranks in brackets)",
            wall.mean()
        );
        let _ = writeln!(
            out,
            "{:>6} {:>7} {:>10} {:>10} {:>9} {:>9} {:>8} {:>8} {:>8} {:>12} {:>12}",
            "line", "calls", "body", "[max]", "wait", "gather", "encode", "wire", "decode", "sent/call", "recv/call"
        );
        let (mut body, mut wait, mut gather) = (0.0, 0.0, 0.0);
        for (line, s) in &sites {
            let calls = s[0].mean().max(1.0);
            let _ = writeln!(
                out,
                "{:>6} {:>7} {:>10.3} {:>10.3} {:>9.3} {:>9.3} {:>8.3} {:>8.3} {:>8.3} {:>12} {:>12}",
                line,
                s[0].mean() as u64,
                s[1].mean(),
                s[1].max,
                s[2].mean(),
                s[3].mean(),
                s[4].mean(),
                s[5].mean(),
                s[6].mean(),
                bytes(s[7].mean() / calls),
                bytes(s[8].mean() / calls),
            );
            body += s[1].mean();
            wait += s[2].mean();
            gather += s[3].mean();
        }
        let _ = writeln!(out, "serial code between top-level PLOOPs (previous line -> next line):");
        let _ = writeln!(out, "{:>6} {:>6} {:>7} {:>10} {:>10}", "from", "to", "count", "seconds", "[max]");
        let mut serial = 0.0;
        for ((prev, next), s) in &gaps {
            let _ = writeln!(
                out,
                "{:>6} {:>6} {:>7} {:>10.3} {:>10.3}",
                prev,
                next,
                s[0].mean() as u64,
                s[1].mean(),
                s[1].max
            );
            serial += s[1].mean();
        }
        let _ = writeln!(
            out,
            "totals: ploop bodies {body:.2}s, waiting on slowest rank {wait:.2}s, gathers {gather:.2}s, between ploops {serial:.2}s"
        );
        out
    }

    fn bytes(v: f64) -> String {
        if v >= 1e9 {
            format!("{:.2} GB", v / 1e9)
        } else if v >= 1e6 {
            format!("{:.2} MB", v / 1e6)
        } else if v >= 1e3 {
            format!("{:.1} KB", v / 1e3)
        } else {
            format!("{v:.0} B")
        }
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
