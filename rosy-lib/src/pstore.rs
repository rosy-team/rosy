//! Particles spread across MPI ranks by Morton key.
//!
//! Each rank owns one contiguous key range and keeps its particles sorted
//! by `(key, id)`. No rank holds or loops over all particles. The leaf set
//! is the one [`crate::fmmtree::LeafTable`] builds on a single process, for
//! any rank count. See `docs/distributed-storage.md`.

use std::collections::BTreeSet;
// The store hashes box and particle numbers it computes itself, so the fast
// non-randomised hasher is safe, and its fixed order keeps runs repeatable.
type HashMap<K, V> = rustc_hash::FxHashMap<K, V>;
type HashSet<K> = rustc_hash::FxHashSet<K>;

use anyhow::{Result, bail, ensure};
use mpi::collective::SystemOperation;
use mpi::datatype::{Partition, PartitionMut};
use mpi::topology::SimpleCommunicator;
use mpi::traits::*;

use crate::fmmtree::{
    Bounds, FINEST_LEVEL, Leaf, box_centre, box_number, key_range, morton_key, neighbours,
};

/// Values carried per particle: x y z px py pz f n e. `e` is the program's
/// own per-particle tag (PHAD keeps each particle's lattice element there);
/// the store moves it with the particle and never reads it.
pub const FIELDS: usize = 9;
/// Fields `PSPUT` writes back: positions, momenta and `e`.
const PUT_FIELDS: [usize; 7] = [0, 1, 2, 3, 4, 5, 8];
/// Keys offered per rank, on average, when the ranges are chosen. A rank
/// offers in proportion to the particles it holds.
const SAMPLES_PER_RANK: usize = 128;
/// Per-particle weight of the upward and downward passes, on the scale
/// where a leaf's near-field work is its neighbourhood size squared.
const PASS_WEIGHT: u64 = 400;
/// Measured weights over all ranks add up to about this.
const WEIGHT_SCALE: f64 = 1e15;
/// Deterministic balancing: nominal seconds per expansion translation
/// (M2M, M2L, L2L, or one particle summed into a multipole), so counted
/// pass work and the program's counted near-field work share a scale.
/// `ROSY_PS_TRANSLATION_SECONDS` overrides it.
const TRANSLATION_SECONDS: f64 = 2e-6;

/// Whether ranks are balanced by counted work only (`ROSY_PS_DETERMINISTIC`
/// set), so the same inputs on the same number of ranks give the same
/// particle split, and the same results, every run. Otherwise measured
/// times are used, which follow the machine more closely but differ from
/// run to run.
pub fn deterministic() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("ROSY_PS_DETERMINISTIC").is_some())
}

fn translation_seconds() -> f64 {
    static V: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("ROSY_PS_TRANSLATION_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(TRANSLATION_SECONDS)
    })
}
/// Levels at or above this many boxes are counted by query, not by a
/// dense histogram: 8^6 boxes is 2 MB per rank.
const DENSE_COUNT_LEVELS: u32 = 6;

/// The beam's rest frame, as PHAD's `SC_A` uses it: the common velocity
/// `W = gamma * beta` and `relg = gamma`, both as PHAD computed them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub w: [f64; 3],
    pub relg: f64,
}

impl Default for Frame {
    /// At rest: positions unchanged.
    fn default() -> Self {
        Frame { w: [0.0; 3], relg: 1.0 }
    }
}

impl Frame {
    /// A lab position in the rest frame, in the exact operations of
    /// PHAD's `SC_A`: `DOT := (WX*X + WY*Y + WZ*Z)/(RELG+1)`, then
    /// `AX := X + WX*DOT` and the same for y and z.
    #[inline]
    pub fn boost(&self, x: f64, y: f64, z: f64) -> [f64; 3] {
        let [wx, wy, wz] = self.w;
        let dot = (wx * x + wy * y + wz * z) / (self.relg + 1.0);
        [x + wx * dot, y + wy * dot, z + wz * dot]
    }
}

/// One rank's share of the particles.
#[derive(Debug, Default)]
pub struct ParticleStore {
    /// Original particle number. It never changes.
    pub id: Vec<u64>,
    pub key: Vec<u64>,
    pub data: Vec<[f64; FIELDS]>,
    /// Level of the leaf each particle sits in.
    pub leaf_level: Vec<u8>,
    /// Work each particle costs per step, from its last leaf: measured
    /// nanoseconds once the program reports costs, a model before that.
    /// Ranks are balanced by this, not by particle count.
    pub weight: Vec<u64>,
    /// Seconds the program has charged to each owned leaf since the last
    /// rebuild (`PSCOST`).
    pub leaf_cost: Vec<f64>,
    /// Seconds this rank spent computing expansions since the last
    /// rebuild: the time between each `PSUP` or `PSDOWN` and the sync
    /// that follows it.
    pub pass_seconds: f64,
    pass_mark: Option<std::time::Instant>,
    /// Seconds spent inside each intrinsic, in [`PS_INTRINSICS`] order, then
    /// the parts of `PSBUILD` in [`BUILD_PARTS`] order. For the stats report.
    pub call_seconds: Vec<f64>,
    /// Weights are measured costs, on every rank.
    measured: bool,
    /// Leaves owned by this rank, in key order. `first` is a local slot.
    pub leaves: Vec<Leaf>,
    /// `splitters[r]..splitters[r + 1]` is the key range of rank `r`.
    pub splitters: Vec<u64>,
    pub bounds: Option<Bounds>,
    /// Copies of other ranks' particles in the boxes around this rank's
    /// leaves, sorted by `(key, id)`. Filled by
    /// [`ParticleStore::exchange_halo`]; position `len() + i` names entry `i`.
    pub halo_id: Vec<u64>,
    pub halo_key: Vec<u64>,
    pub halo_data: Vec<[f64; FIELDS]>,
    /// Particles handed to `PSLOAD` so far, over all ranks.
    pub loaded: u64,
    /// This rank's part of the upward and downward passes.
    pub plan: Plan,
    /// The rest frame of the last rebuild. Keys, the bounding cube and
    /// `PSSCALED` are in this frame; the stored positions stay in the lab.
    pub frame: Frame,
    /// For each owned particle, the particle numbers that were in its
    /// leaf's neighbourhood (the leaf and the boxes around it) before the
    /// last rebuild, ascending. Empty until the second rebuild.
    pub prev: Vec<Vec<u64>>,
    /// Copies of particles named in `prev` that are neither owned nor in
    /// the halo. Position `len() + halo + i` names entry `i`.
    pub far_id: Vec<u64>,
    pub far_data: Vec<[f64; FIELDS]>,
    /// Local position of every particle held here, by particle number.
    position_of: HashMap<u64, usize>,
    /// For the particle numbers this rank keeps the directory of: the
    /// rank that owns each one.
    directory: HashMap<u64, u64>,
}

/// One rank's share of the passes. A box belongs to the rank whose key
/// range holds the box's first key. Lists are sorted and indexed by level.
#[derive(Debug, Default)]
pub struct Plan {
    /// Number of levels: the deepest leaf level over all ranks, plus one.
    pub depth: usize,
    /// Owned boxes that get a local expansion.
    pub targets: Vec<Vec<u64>>,
    /// Owned targets that had to divide.
    pub divided: Vec<Vec<u64>>,
    /// Owned boxes whose multipole some rank needs.
    pub multipoles: Vec<Vec<u64>>,
    /// Multipoles this rank needs from other ranks: `(level, number)`.
    pub ghost_multipoles: BTreeSet<(u32, u64)>,
    /// Ranks that need a copy of an owned multipole: `(level, number, rank)`.
    pub multipole_users: Vec<(u32, u64, usize)>,
    /// Key ranges of boxes whose particles an owned multipole sums
    /// directly. The halo exchange fetches the parts other ranks hold.
    pub extra_ranges: Vec<(u64, u64)>,
    /// Parents' local expansions this rank needs from other ranks.
    pub ghost_locals: BTreeSet<(u32, u64)>,
    /// Ranks that need a copy of an owned local expansion.
    pub local_users: Vec<(u32, u64, usize)>,
    /// Position (from 0) of each multipole in this rank's array: owned
    /// ones in `(level, number)` order, then copies.
    pub multipole_index: HashMap<(u32, u64), usize>,
    /// The same for local expansions.
    pub local_index: HashMap<(u32, u64), usize>,
    /// Particle counts over all ranks, for the boxes this rank asked about.
    counts: HashMap<(u32, u64), u64>,
    /// Boxes owned elsewhere that are known to have divided.
    remote_divided: HashSet<(u32, u64)>,
    /// Each target's interaction list, in `targets` order, worked out once
    /// per build.
    interactions: Vec<Vec<Vec<u64>>>,
}

/// How an owned multipole is put together. See
/// [`crate::fmmtree::MultipoleRecipe`]; particles are looked up by box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoxRecipe {
    Particles,
    Children { children: Vec<u64>, direct: Vec<u64> },
}

impl ParticleStore {
    pub fn len(&self) -> usize {
        self.id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.id.is_empty()
    }

    /// Add a particle on this rank. It moves to its owner at the next
    /// [`ParticleStore::rebuild`].
    pub fn push(&mut self, id: u64, data: [f64; FIELDS]) {
        self.id.push(id);
        self.key.push(0);
        self.data.push(data);
        self.leaf_level.push(0);
        self.weight.push(1);
        self.prev.push(Vec::new());
    }

    /// Recompute keys from the current positions, move every particle to
    /// the rank that owns its key, and rebuild the leaves.
    ///
    /// Keys are taken from positions boosted into `frame` (PHAD's `SC_A`).
    /// `q` is the most particles allowed around one leaf.
    pub fn rebuild(&mut self, world: &SimpleCommunicator, q: usize, frame: Frame) -> Result<()> {
        let ranks = world.size() as usize;
        // Remember each particle's neighbourhood under the old tree.
        for leaf in &self.leaves {
            let mut around: Vec<u64> = self
                .box_positions(leaf.level, leaf.number)
                .into_iter()
                .chain(self.neighbour_positions(leaf))
                .map(|p| self.id_at(p))
                .collect();
            around.sort_unstable();
            for slot in leaf.first..leaf.first + leaf.count {
                self.prev[slot] = around.clone();
            }
        }
        self.far_id.clear();
        self.far_data.clear();
        let bounds = self.global_bounds(world, frame)?;
        self.bounds = Some(bounds);
        self.frame = frame;
        for (key, d) in self.key.iter_mut().zip(&self.data) {
            *key = morton_key(bounds.scale(frame.boost(d[0], d[1], d[2])));
        }

        self.splitters = self.sample_splitters(world)?;
        self.migrate(world)?;
        self.refine(world, q)?;

        // No leaf may straddle two ranks: start each range at the leaf
        // that holds its first particle.
        let first = match self.key.first() {
            Some(&key) => key_range(self.leaf_level[0] as u32, box_number(key, self.leaf_level[0] as u32)).0,
            None => u64::MAX,
        };
        let mut starts = vec![0u64; ranks];
        world.all_gather_into(&first, &mut starts[..]);
        let mut snapped = vec![u64::MAX; ranks + 1];
        for r in (1..ranks).rev() {
            snapped[r] = starts[r].min(snapped[r + 1]);
        }
        snapped[0] = 0;
        self.splitters = snapped;
        self.migrate(world)?;

        self.halo_id.clear();
        self.halo_key.clear();
        self.halo_data.clear();
        self.plan = Plan::default();
        self.leaves.clear();
        let mut at = 0;
        while at < self.key.len() {
            let level = self.leaf_level[at] as u32;
            let number = box_number(self.key[at], level);
            let hi = key_range(level, number).1;
            let count = self.key[at..].partition_point(|&k| k < hi);
            self.leaves.push(Leaf { level, number, first: at, count });
            at += count;
        }

        // Tell the directory where every particle now lives.
        let mut notes: Vec<Vec<u64>> = vec![Vec::new(); ranks];
        for &id in &self.id {
            notes[self.directory_rank(id, ranks)].push(id);
        }
        self.directory.clear();
        for (rank, ids) in exchange(world, &notes)?.into_iter().enumerate() {
            for id in ids {
                self.directory.insert(id, rank as u64);
            }
        }
        Ok(())
    }

    /// Turn the costs measured since the last rebuild into particle
    /// weights. A leaf's cost is shared by its particles; the rank's pass
    /// time is shared by all of its particles. Does nothing until some
    /// rank has reported a cost.
    pub fn apply_costs(&mut self, world: &SimpleCommunicator) -> Result<()> {
        // Counted pass work per owned particle, in seconds at the nominal
        // rate, replaces the measured pass time when balancing is
        // deterministic.
        let counted = if deterministic() && self.leaf_cost.len() == self.leaves.len() {
            let work = self.pass_work();
            self.pass_seconds = work.iter().sum();
            Some(work)
        } else {
            None
        };
        let near: f64 = self.leaf_cost.iter().sum();
        let heaviest = self.leaf_cost.iter().copied().fold(0.0, f64::max);
        let mine = [near, self.pass_seconds, near + self.pass_seconds];
        let mut total = [0.0f64; 3];
        let mut most = [0.0f64; 3];
        world.all_reduce_into(&mine[..], &mut total[..], SystemOperation::sum());
        world.all_reduce_into(&mine[..], &mut most[..], SystemOperation::max());
        let mut heaviest_leaf = 0.0f64;
        world.all_reduce_into(&heaviest, &mut heaviest_leaf, SystemOperation::max());
        if total[0] > 0.0 && self.leaf_cost.len() == self.leaves.len() {
            // A weight is the particle's share of a whole step's work, so a
            // step that costs twenty times more (a beam-line element, a
            // burst of close encounters) counts no more than a quiet one.
            // Shares are averaged with the earlier ones, because the costly
            // steps have a different shape and come back.
            let held: u64 = self.weight.iter().sum();
            let mut all_held = 0u64;
            world.all_reduce_into(&held, &mut all_held, SystemOperation::sum());
            let earlier = if self.measured && all_held > 0 { WEIGHT_SCALE / all_held as f64 } else { 0.0 };
            let now = WEIGHT_SCALE / total[2];
            let blend = if earlier > 0.0 { 0.5 } else { 1.0 };
            let shared = self.pass_seconds / self.len().max(1) as f64;
            for (leaf, &cost) in self.leaves.iter().zip(&self.leaf_cost) {
                let near_share = cost / leaf.count as f64;
                for slot in leaf.first..leaf.first + leaf.count {
                    let pass = counted.as_ref().map_or(shared, |work| work[slot]);
                    let share = (near_share + pass) * now;
                    let w = &mut self.weight[slot];
                    *w = ((blend * share + (1.0 - blend) * *w as f64 * earlier) as u64).max(1);
                }
            }
            self.measured = true;
            if world.rank() == 0 && std::env::var_os("ROSY_MPI_STATS").is_some() {
                let ranks = world.size() as f64;
                eprintln!(
                    "[pstore] step cost per rank, mean [max]: near {:.3} [{:.3}]  passes {:.3} [{:.3}]  both {:.3} [{:.3}]  heaviest leaf {:.3}",
                    total[0] / ranks, most[0], total[1] / ranks, most[1], total[2] / ranks, most[2], heaviest_leaf
                );
            }
        }
        self.leaf_cost.clear();
        self.pass_seconds = 0.0;
        self.pass_mark = None;
        Ok(())
    }

    /// The passes' work, counted from the plan, as seconds at the nominal
    /// rate per owned particle slot: each owned box's translations go to
    /// the particles this rank holds in it (to all of its particles when
    /// it holds none there).
    fn pass_work(&self) -> Vec<f64> {
        let unit = translation_seconds();
        let mut work = vec![0.0f64; self.len()];
        let mut spread = 0.0f64;
        let mut charge = |work: &mut Vec<f64>, level: u32, number: u64, translations: usize| {
            if translations == 0 {
                return;
            }
            let (lo, hi) = key_range(level, number);
            let (first, end) = self.slots(lo, hi);
            let seconds = translations as f64 * unit;
            if end > first {
                let each = seconds / (end - first) as f64;
                for w in &mut work[first..end] {
                    *w += each;
                }
            } else {
                spread += seconds;
            }
        };
        for (level, boxes) in self.plan.targets.iter().enumerate() {
            let level = level as u32;
            for (t, &number) in boxes.iter().enumerate() {
                let translations = self.plan.interactions[level as usize][t].len() + usize::from(level > 2);
                charge(&mut work, level, number, translations);
            }
        }
        for (level, boxes) in self.plan.multipoles.iter().enumerate() {
            let level = level as u32;
            for &number in boxes {
                let translations = match self.multipole_recipe(level, number) {
                    BoxRecipe::Particles => self.box_held(level, number),
                    BoxRecipe::Children { children, direct } => {
                        children.len() + direct.iter().map(|&c| self.box_held(level + 1, c)).sum::<usize>()
                    }
                };
                charge(&mut work, level, number, translations);
            }
        }
        if spread > 0.0 && !work.is_empty() {
            let each = spread / work.len() as f64;
            for w in &mut work {
                *w += each;
            }
        }
        work
    }

    /// Rank that keeps the directory entry of particle `id`.
    fn directory_rank(&self, id: u64, ranks: usize) -> usize {
        let total = self.loaded.max(1) as u128;
        ((id as u128 * ranks as u128 / total) as usize).min(ranks - 1)
    }

    /// Fetch copies of the particles in `prev` lists that this rank does
    /// not already hold, and index every held particle by number. Call it
    /// after [`ParticleStore::exchange_halo`].
    pub fn fetch_previous(&mut self, world: &SimpleCommunicator) -> Result<()> {
        let ranks = world.size() as usize;
        let mut here: HashSet<u64> = self.id.iter().chain(&self.halo_id).copied().collect();
        let mut missing: Vec<Vec<u64>> = vec![Vec::new(); ranks];
        for list in &self.prev {
            for &id in list {
                if here.insert(id) {
                    missing[self.directory_rank(id, ranks)].push(id);
                }
            }
        }
        // Ask the directory who owns each one, then ask the owners.
        let asked = exchange(world, &missing)?;
        let answers: Vec<Vec<u64>> = asked
            .iter()
            .map(|ids| ids.iter().map(|id| self.directory.get(id).copied().unwrap_or(u64::MAX)).collect())
            .collect();
        let owners = exchange(world, &answers)?;
        let mut asks: Vec<Vec<u64>> = vec![Vec::new(); ranks];
        for (ids, owners) in missing.iter().zip(&owners) {
            for (&id, &owner) in ids.iter().zip(owners) {
                ensure!(owner != u64::MAX, "particle {id} is not in the directory");
                asks[owner as usize].push(id);
            }
        }
        let slot_of: HashMap<u64, usize> = self.id.iter().enumerate().map(|(i, &id)| (id, i)).collect();
        let asked = exchange(world, &asks)?;
        let mut values: Vec<Vec<f64>> = vec![Vec::new(); ranks];
        for (rank, ids) in asked.iter().enumerate() {
            for id in ids {
                let slot = *slot_of.get(id).ok_or_else(|| anyhow::anyhow!("particle {id} is not on the rank the directory names"))?;
                values[rank].extend(self.data[slot]);
            }
        }
        let replies = exchange(world, &values)?;
        self.far_id.clear();
        self.far_data.clear();
        for (ids, reply) in asks.iter().zip(&replies) {
            ensure!(reply.len() == ids.len() * FIELDS, "far particle fetch lost data");
            self.far_id.extend(ids);
            self.far_data.extend(reply.chunks_exact(FIELDS).map(|v| <[f64; FIELDS]>::try_from(v).unwrap()));
        }
        self.position_of = self
            .id
            .iter()
            .chain(&self.halo_id)
            .chain(&self.far_id)
            .enumerate()
            .map(|(i, &id)| (id, i))
            .collect();
        Ok(())
    }

    /// Fetch the particles other ranks hold in the boxes around this
    /// rank's leaves. Call it after [`ParticleStore::rebuild`], and again
    /// whenever the values those copies carry have changed.
    pub fn exchange_halo(&mut self, world: &SimpleCommunicator) -> Result<()> {
        let ranks = world.size() as usize;
        let me = world.rank() as usize;
        let (my_lo, my_hi) = (self.splitters[me], self.splitters[me + 1]);

        // Key ranges wanted from each rank, merged so no particle is sent twice.
        let mut wanted: Vec<Vec<(u64, u64)>> = vec![Vec::new(); ranks];
        for leaf in &self.leaves {
            for number in neighbours(leaf.level, leaf.number) {
                let (lo, hi) = key_range(leaf.level, number);
                if lo >= my_lo && hi <= my_hi {
                    continue;
                }
                for rank in self.owner(lo)..=self.owner(hi - 1) {
                    if rank != me {
                        wanted[rank].push((lo, hi));
                    }
                }
            }
        }
        for &(lo, hi) in &self.plan.extra_ranges {
            for rank in self.owner(lo)..=self.owner(hi - 1) {
                if rank != me {
                    wanted[rank].push((lo, hi));
                }
            }
        }
        let asks: Vec<Vec<u64>> = wanted
            .into_iter()
            .map(|mut ranges| {
                ranges.sort_unstable();
                let mut flat: Vec<u64> = Vec::new();
                for (lo, hi) in ranges {
                    match flat.last_mut() {
                        Some(end) if lo <= *end => *end = (*end).max(hi),
                        _ => flat.extend([lo, hi]),
                    }
                }
                flat
            })
            .collect();

        let asked = exchange(world, &asks)?;
        let mut words: Vec<Vec<u64>> = vec![Vec::new(); ranks];
        let mut values: Vec<Vec<f64>> = vec![Vec::new(); ranks];
        for (rank, ranges) in asked.iter().enumerate() {
            for range in ranges.chunks_exact(2) {
                let (first, end) = self.slots(range[0], range[1]);
                for slot in first..end {
                    words[rank].extend([self.id[slot], self.key[slot]]);
                    values[rank].extend(self.data[slot]);
                }
            }
        }
        // Ranks are in key order and each reply is in key order, so the
        // concatenation is sorted.
        let words: Vec<u64> = exchange(world, &words)?.into_iter().flatten().collect();
        let values: Vec<f64> = exchange(world, &values)?.into_iter().flatten().collect();
        ensure!(values.len() == words.len() / 2 * FIELDS, "halo exchange lost data");
        self.halo_id = words.chunks_exact(2).map(|w| w[0]).collect();
        self.halo_key = words.chunks_exact(2).map(|w| w[1]).collect();
        self.halo_data = values
            .chunks_exact(FIELDS)
            .map(|v| v.try_into().unwrap())
            .collect();

        // Near-field work grows with the square of the neighbourhood and
        // is shared by the leaf's particles; the passes cost about the
        // same for every particle. The constant is the measured ratio of
        // the two in PHAD.
        if self.measured {
            return Ok(());
        }
        for leaf in &self.leaves {
            let around = (leaf.count + self.neighbour_positions(leaf).len()) as u64;
            let weight = around * around / leaf.count as u64 + PASS_WEIGHT;
            self.weight[leaf.first..leaf.first + leaf.count].fill(weight);
        }
        Ok(())
    }

    /// Particle number at a local position, owned or halo.
    pub fn id_at(&self, position: usize) -> u64 {
        match position.checked_sub(self.len()) {
            Some(i) if i >= self.halo_id.len() => self.far_id[i - self.halo_id.len()],
            Some(i) => self.halo_id[i],
            None => self.id[position],
        }
    }

    /// Positions of the particles in box `(level, number)`, in particle
    /// number order. Owned particles and halo copies are both found.
    pub fn box_positions(&self, level: u32, number: u64) -> Vec<usize> {
        let (lo, hi) = key_range(level, number);
        let (first, end) = self.slots(lo, hi);
        let mut out: Vec<usize> = (first..end).collect();
        let h0 = self.halo_key.partition_point(|&k| k < lo);
        let h1 = self.halo_key.partition_point(|&k| k < hi);
        out.extend((h0..h1).map(|i| self.len() + i));
        out.sort_unstable_by_key(|&p| self.id_at(p));
        out
    }

    /// How many particles of a box this rank holds, owned or halo: the
    /// length of `box_positions` without building it.
    fn box_held(&self, level: u32, number: u64) -> usize {
        let (lo, hi) = key_range(level, number);
        let (first, end) = self.slots(lo, hi);
        let h0 = self.halo_key.partition_point(|&k| k < lo);
        let h1 = self.halo_key.partition_point(|&k| k < hi);
        (end - first) + (h1 - h0)
    }

    /// Positions of the particles in the boxes around `leaf`, the leaf
    /// itself left out, in the order `fmmcpp` lists them.
    pub fn neighbour_positions(&self, leaf: &Leaf) -> Vec<usize> {
        let mut out = Vec::new();
        for number in neighbours(leaf.level, leaf.number) {
            if number != leaf.number {
                out.extend(self.box_positions(leaf.level, number));
            }
        }
        out
    }

    /// Rank that owns box `(level, number)`.
    pub fn box_owner(&self, level: u32, number: u64) -> usize {
        self.owner(key_range(level, number).0)
    }

    /// Send `(level, number)` pairs to ranks and get one answer per pair.
    fn query(
        &self,
        world: &SimpleCommunicator,
        asks: &[Vec<u64>],
        answer: impl Fn(&Self, u32, u64) -> u64,
    ) -> Result<Vec<Vec<u64>>> {
        let asked = exchange(world, asks)?;
        let answers: Vec<Vec<u64>> = asked
            .iter()
            .map(|pairs| pairs.chunks_exact(2).map(|p| answer(self, p[0] as u32, p[1])).collect())
            .collect();
        let replies = exchange(world, &answers)?;
        for (ask, reply) in asks.iter().zip(&replies) {
            ensure!(ask.len() == 2 * reply.len(), "box query reply has the wrong length");
        }
        Ok(replies)
    }

    /// Particle counts over all ranks for `boxes`, added to the cache.
    fn count_boxes(&mut self, world: &SimpleCommunicator, boxes: &[(u32, u64)]) -> Result<()> {
        let ranks = world.size() as usize;
        let me = world.rank() as usize;
        let mut asks: Vec<Vec<u64>> = vec![Vec::new(); ranks];
        let mut fresh = Vec::new();
        for &(level, number) in boxes {
            if self.plan.counts.contains_key(&(level, number)) {
                continue;
            }
            let (lo, hi) = key_range(level, number);
            let (first, end) = self.slots(lo, hi);
            self.plan.counts.insert((level, number), (end - first) as u64);
            fresh.push((level, number));
            for rank in self.owner(lo)..=self.owner(hi - 1) {
                if rank != me {
                    asks[rank].extend([level as u64, number]);
                }
            }
        }
        let replies = self.query(world, &asks, |store, level, number| {
            let (lo, hi) = key_range(level, number);
            let (first, end) = store.slots(lo, hi);
            (end - first) as u64
        })?;
        for (ask, reply) in asks.iter().zip(&replies) {
            for (pair, count) in ask.chunks_exact(2).zip(reply) {
                *self.plan.counts.get_mut(&(pair[0] as u32, pair[1])).unwrap() += count;
            }
        }
        Ok(())
    }

    fn count(&self, level: u32, number: u64) -> u64 {
        self.plan.counts.get(&(level, number)).copied().unwrap_or(0)
    }

    /// Children of the parent's neighbours that do not touch the box, in
    /// the order of `neighbours(level - 1, parent)` and then child number.
    /// Worked out on cell coordinates: child `c` of the box at cell `p` is
    /// at `2p + ((c >> 2) & 1, (c >> 1) & 1, c & 1)`.
    fn interaction_candidates(level: u32, number: u64) -> Vec<u64> {
        let cell = crate::fmmtree::box_cell(level, number);
        let parent = [cell[0] >> 1, cell[1] >> 1, cell[2] >> 1];
        let last = (1u64 << (level - 1)) - 1;
        let span = |c: u64| -> (u64, u64) { (c.saturating_sub(1), if c == last { last } else { c + 1 }) };
        let (sx, sy, sz) = (span(parent[0]), span(parent[1]), span(parent[2]));
        let near = |a: u64, b: u64| a.abs_diff(b) <= 1;
        let mut out = Vec::with_capacity(189);
        for i in sx.0..=sx.1 {
            for j in sy.0..=sy.1 {
                for k in sz.0..=sz.1 {
                    let near_parent = crate::fmmtree::interleave_cell([i, j, k], level - 1);
                    for c in 0..8u64 {
                        let child = [2 * i + ((c >> 2) & 1), 2 * j + ((c >> 1) & 1), 2 * k + (c & 1)];
                        if !(near(child[0], cell[0]) && near(child[1], cell[1]) && near(child[2], cell[2])) {
                            out.push(near_parent * 8 + c);
                        }
                    }
                }
            }
        }
        out
    }

    /// Boxes whose multipoles feed the local expansion of an owned
    /// target, in `fmmcpp`'s order.
    pub fn interaction_list(&self, level: u32, number: u64) -> Vec<u64> {
        Self::interaction_candidates(level, number)
            .into_iter()
            .filter(|&c| self.count(level, c) > 0)
            .collect()
    }

    /// True when the octree split the box. Valid for owned multipoles.
    pub fn is_split(&self, level: u32, number: u64) -> bool {
        if level < 2 {
            return true;
        }
        let me_divided = self.plan.divided.get(level as usize);
        neighbours(level, number).iter().any(|&n| {
            me_divided.is_some_and(|d| d.binary_search(&n).is_ok())
                || self.plan.remote_divided.contains(&(level, n))
        })
    }

    /// True when box `(level, number)` has a multipole, here or as a copy.
    pub fn has_multipole(&self, level: u32, number: u64) -> bool {
        self.plan
            .multipoles
            .get(level as usize)
            .is_some_and(|m| m.binary_search(&number).is_ok())
            || self.plan.ghost_multipoles.contains(&(level, number))
    }

    /// How an owned multipole is put together.
    pub fn multipole_recipe(&self, level: u32, number: u64) -> BoxRecipe {
        if !self.is_split(level, number) {
            return BoxRecipe::Particles;
        }
        let (mut children, mut direct) = (Vec::new(), Vec::new());
        for child in number * 8..number * 8 + 8 {
            if self.count(level + 1, child) == 0 {
                continue;
            }
            if self.has_multipole(level + 1, child) {
                children.push(child);
            } else {
                direct.push(child);
            }
        }
        BoxRecipe::Children { children, direct }
    }

    /// Work out this rank's share of the upward and downward passes.
    /// Call it after [`ParticleStore::rebuild`] and before
    /// [`ParticleStore::exchange_halo`], on every rank.
    pub fn build_plan(&mut self, world: &SimpleCommunicator) -> Result<()> {
        let ranks = world.size() as usize;
        let me = world.rank() as usize;
        let local_depth = self.leaves.iter().map(|l| l.level).max().unwrap_or(0) + 1;
        let mut depth = 0u32;
        world.all_reduce_into(&local_depth, &mut depth, SystemOperation::max());
        let depth = depth as usize;
        self.plan = Plan { depth, ..Plan::default() };

        // Targets: owned leaves, and every ancestor on the rank that owns it.
        let mut targets = vec![BTreeSet::new(); depth];
        let mut divided = vec![BTreeSet::new(); depth];
        let mut notices: Vec<Vec<u64>> = vec![Vec::new(); ranks];
        let mut seen = HashSet::default();
        for leaf in &self.leaves {
            targets[leaf.level as usize].insert(leaf.number);
            let mut number = leaf.number;
            for level in (2..leaf.level).rev() {
                number >>= 3;
                if !seen.insert((level, number)) {
                    break;
                }
                let owner = self.box_owner(level, number);
                if owner == me {
                    divided[level as usize].insert(number);
                    targets[level as usize].insert(number);
                } else {
                    notices[owner].extend([level as u64, number]);
                }
            }
        }
        for pairs in exchange(world, &notices)? {
            for pair in pairs.chunks_exact(2) {
                divided[pair[0] as usize].insert(pair[1]);
                targets[pair[0] as usize].insert(pair[1]);
            }
        }
        let flat = |sets: Vec<BTreeSet<u64>>| -> Vec<Vec<u64>> {
            sets.into_iter().map(|s| s.into_iter().collect()).collect()
        };
        self.plan.targets = flat(targets);
        self.plan.divided = flat(divided);

        // Which boxes around each target hold particles.
        let mut wanted = HashSet::default();
        for (level, boxes) in self.plan.targets.iter().enumerate() {
            for &number in boxes {
                for c in Self::interaction_candidates(level as u32, number) {
                    wanted.insert((level as u32, c));
                }
            }
        }
        let wanted: Vec<(u32, u64)> = wanted.into_iter().collect();
        self.count_boxes(world, &wanted)?;

        // Every target's interaction list, kept for the passes.
        let interactions: Vec<Vec<Vec<u64>>> = self
            .plan
            .targets
            .iter()
            .enumerate()
            .map(|(level, boxes)| boxes.iter().map(|&number| self.interaction_list(level as u32, number)).collect())
            .collect();
        self.plan.interactions = interactions;

        // Multipoles: tell each owner which of its boxes are needed.
        let mut multipoles = vec![BTreeSet::new(); depth];
        let mut requests: Vec<Vec<u64>> = vec![Vec::new(); ranks];
        for (level, boxes) in self.plan.targets.iter().enumerate() {
            let level = level as u32;
            for (t, &_number) in boxes.iter().enumerate() {
                for &c in &self.plan.interactions[level as usize][t] {
                    let owner = self.box_owner(level, c);
                    if owner == me {
                        multipoles[level as usize].insert(c);
                    } else if self.plan.ghost_multipoles.insert((level, c)) {
                        requests[owner].extend([level as u64, c]);
                    }
                }
            }
        }
        for (rank, pairs) in exchange(world, &requests)?.into_iter().enumerate() {
            for pair in pairs.chunks_exact(2) {
                multipoles[pair[0] as usize].insert(pair[1]);
                self.plan.multipole_users.push((pair[0] as u32, pair[1], rank));
            }
        }
        self.plan.multipoles = flat(multipoles);

        // What each owned multipole is built from.
        // 1. Whether boxes next to it, owned elsewhere, divided.
        let mut asks: Vec<Vec<u64>> = vec![Vec::new(); ranks];
        let mut asked = HashSet::default();
        for (level, boxes) in self.plan.multipoles.iter().enumerate() {
            let level = level as u32;
            if level < 2 {
                continue;
            }
            for &number in boxes {
                for n in neighbours(level, number) {
                    let owner = self.box_owner(level, n);
                    if owner != me && asked.insert((level, n)) {
                        asks[owner].extend([level as u64, n]);
                    }
                }
            }
        }
        let replies = self.query(world, &asks, |store, level, number| {
            store.plan.divided.get(level as usize).is_some_and(|d| d.binary_search(&number).is_ok()) as u64
        })?;
        for (ask, reply) in asks.iter().zip(&replies) {
            for (pair, &yes) in ask.chunks_exact(2).zip(reply) {
                if yes != 0 {
                    self.plan.remote_divided.insert((pair[0] as u32, pair[1]));
                }
            }
        }
        // 2. Which of its children hold particles.
        let mut children = Vec::new();
        for (level, boxes) in self.plan.multipoles.iter().enumerate() {
            let level = level as u32;
            for &number in boxes {
                if self.is_split(level, number) {
                    children.extend((number * 8..number * 8 + 8).map(|c| (level + 1, c)));
                }
            }
        }
        self.count_boxes(world, &children)?;
        // 3. Which of those children have a multipole on another rank.
        let mut asks: Vec<Vec<u64>> = vec![Vec::new(); ranks];
        for &(level, child) in &children {
            let owner = self.box_owner(level, child);
            if owner != me && self.count(level, child) > 0 && !self.plan.ghost_multipoles.contains(&(level, child)) {
                asks[owner].extend([level as u64, child]);
            }
        }
        let replies = self.query(world, &asks, |store, level, number| {
            store.plan.multipoles.get(level as usize).is_some_and(|m| m.binary_search(&number).is_ok()) as u64
        })?;
        let mut requests: Vec<Vec<u64>> = vec![Vec::new(); ranks];
        for (rank, (ask, reply)) in asks.iter().zip(&replies).enumerate() {
            for (pair, &yes) in ask.chunks_exact(2).zip(reply) {
                if yes != 0 && self.plan.ghost_multipoles.insert((pair[0] as u32, pair[1])) {
                    requests[rank].extend([pair[0], pair[1]]);
                }
            }
        }
        for (rank, pairs) in exchange(world, &requests)?.into_iter().enumerate() {
            for pair in pairs.chunks_exact(2) {
                self.plan.multipole_users.push((pair[0] as u32, pair[1], rank));
            }
        }
        // 4. Children summed particle by particle may lie on other ranks.
        let (my_lo, my_hi) = (self.splitters[me], self.splitters[me + 1]);
        for &(level, child) in &children {
            if self.count(level, child) > 0 && !self.has_multipole(level, child) {
                let (lo, hi) = key_range(level, child);
                if lo < my_lo || hi > my_hi {
                    self.plan.extra_ranges.push((lo, hi));
                }
            }
        }

        // A target shifts its parent's local expansion. Ask for parents
        // that live on another rank.
        let mut requests: Vec<Vec<u64>> = vec![Vec::new(); ranks];
        for (level, boxes) in self.plan.targets.iter().enumerate().skip(3) {
            let parent_level = level as u32 - 1;
            for &number in boxes {
                let parent = number >> 3;
                let owner = self.box_owner(parent_level, parent);
                if owner != me && self.plan.ghost_locals.insert((parent_level, parent)) {
                    requests[owner].extend([parent_level as u64, parent]);
                }
            }
        }
        for (rank, pairs) in exchange(world, &requests)?.into_iter().enumerate() {
            for pair in pairs.chunks_exact(2) {
                self.plan.local_users.push((pair[0] as u32, pair[1], rank));
            }
        }

        let number_boxes = |owned: &[Vec<u64>], ghosts: &BTreeSet<(u32, u64)>| {
            let owned = owned
                .iter()
                .enumerate()
                .flat_map(|(level, boxes)| boxes.iter().map(move |&n| (level as u32, n)));
            owned.chain(ghosts.iter().copied()).enumerate().map(|(i, b)| (b, i)).collect()
        };
        self.plan.multipole_index = number_boxes(&self.plan.multipoles, &self.plan.ghost_multipoles);
        self.plan.local_index = number_boxes(&self.plan.targets, &self.plan.ghost_locals);
        Ok(())
    }

    /// Work list for the upward pass at one level: one record per owned
    /// multipole. Positions and indices count from 1.
    ///
    /// `index cx cy cz kind`, then for kind 0 (sum the box's particles)
    /// `n position...`; for kind 1 `nc (child_index cx cy cz)...` to shift,
    /// then `nd (n position...)...` to sum about this box's centre.
    pub fn upward_list(&self, level: u32) -> (Vec<f64>, usize) {
        let mut out = Vec::new();
        let boxes = self.plan.multipoles.get(level as usize).map(|b| &b[..]).unwrap_or(&[]);
        let positions = |out: &mut Vec<f64>, level: u32, number: u64| {
            let list = self.box_positions(level, number);
            out.push(list.len() as f64);
            out.extend(list.iter().map(|&p| (p + 1) as f64));
        };
        for &number in boxes {
            out.push((self.plan.multipole_index[&(level, number)] + 1) as f64);
            out.extend(box_centre(level, number));
            match self.multipole_recipe(level, number) {
                BoxRecipe::Particles => {
                    out.push(0.0);
                    positions(&mut out, level, number);
                }
                BoxRecipe::Children { children, direct } => {
                    out.push(1.0);
                    out.push(children.len() as f64);
                    for child in children {
                        out.push((self.plan.multipole_index[&(level + 1, child)] + 1) as f64);
                        out.extend(box_centre(level + 1, child));
                    }
                    out.push(direct.len() as f64);
                    for child in direct {
                        positions(&mut out, level + 1, child);
                    }
                }
            }
        }
        (out, boxes.len())
    }

    /// Work list for the downward pass at one level: one record per owned
    /// target. `index cx cy cz parent_index pcx pcy pcz`, with parent
    /// index 0 at level 2, then `n (multipole_index cx cy cz)...`.
    pub fn downward_list(&self, level: u32) -> (Vec<f64>, usize) {
        let mut out = Vec::new();
        let boxes = self.plan.targets.get(level as usize).map(|b| &b[..]).unwrap_or(&[]);
        for (t, &number) in boxes.iter().enumerate() {
            out.push((self.plan.local_index[&(level, number)] + 1) as f64);
            out.extend(box_centre(level, number));
            if level > 2 {
                out.push((self.plan.local_index[&(level - 1, number >> 3)] + 1) as f64);
                out.extend(box_centre(level - 1, number >> 3));
            } else {
                out.extend([0.0; 4]);
            }
            let list = &self.plan.interactions[level as usize][t];
            out.push(list.len() as f64);
            for &other in list {
                out.push((self.plan.multipole_index[&(level, other)] + 1) as f64);
                out.extend(box_centre(level, other));
            }
        }
        (out, boxes.len())
    }

    /// Copy this level's owned expansions to the ranks that asked for
    /// them, and store the copies other ranks send here.
    fn sync_cells(
        &self,
        world: &SimpleCommunicator,
        cells: &mut [crate::RosyValue],
        level: u32,
        users: &[(u32, u64, usize)],
        ghosts: &BTreeSet<(u32, u64)>,
        index: &HashMap<(u32, u64), usize>,
    ) -> Result<[f64; 4]> {
        let ranks = world.size() as usize;
        ensure!(cells.len() >= index.len(), "expansion array holds {} cells but {} are needed", cells.len(), index.len());
        // With stats on, a barrier first separates waiting for the slowest
        // rank from the exchange itself.
        let mut parts = [0.0f64; 4];
        let mut mark = std::time::Instant::now();
        let mut lap = |part: usize| {
            parts[part] += mark.elapsed().as_secs_f64();
            mark = std::time::Instant::now();
        };
        if stats_on() {
            world.barrier();
        }
        lap(0);
        let config = bincode::config::standard();
        let mut outgoing: Vec<Vec<u64>> = vec![Vec::new(); ranks];
        for &(l, number, rank) in users {
            if l == level {
                outgoing[rank].push(number);
            }
        }
        let mut bytes: Vec<Vec<u8>> = Vec::with_capacity(ranks);
        for numbers in outgoing.iter_mut() {
            numbers.sort_unstable();
            let values: Vec<&crate::RosyValue> =
                numbers.iter().map(|&n| &cells[index[&(level, n)]]).collect();
            bytes.push(bincode::encode_to_vec(&values, config)?);
        }
        lap(1);
        let received = exchange(world, &bytes)?;
        lap(2);
        let mut expected: Vec<Vec<u64>> = vec![Vec::new(); ranks];
        for &(_, number) in ghosts.range((level, 0)..(level + 1, 0)) {
            expected[self.box_owner(level, number)].push(number);
        }
        for (numbers, bytes) in expected.iter().zip(&received) {
            let (values, _): (Vec<crate::RosyValue>, _) = bincode::decode_from_slice(bytes, config)?;
            ensure!(values.len() == numbers.len(), "expansion exchange sent {} values for {} boxes", values.len(), numbers.len());
            for (&number, value) in numbers.iter().zip(values) {
                cells[index[&(level, number)]] = value;
            }
        }
        lap(3);
        Ok(parts)
    }

    fn add_sync_parts(&mut self, parts: [f64; 4]) {
        let first = PS_INTRINSICS.len() + 4;
        self.call_seconds.resize(PS_INTRINSICS.len() + BUILD_PARTS.len(), 0.0);
        for (slot, part) in self.call_seconds[first..first + 4].iter_mut().zip(parts) {
            *slot += part;
        }
    }

    /// Rank that owns `key` under the current splitters.
    pub fn owner(&self, key: u64) -> usize {
        let inner = &self.splitters[1..self.splitters.len() - 1];
        inner.partition_point(|&s| s <= key)
    }

    /// Local slots whose keys fall in `lo..hi`.
    fn slots(&self, lo: u64, hi: u64) -> (usize, usize) {
        (
            self.key.partition_point(|&k| k < lo),
            self.key.partition_point(|&k| k < hi),
        )
    }

    fn global_bounds(&self, world: &SimpleCommunicator, frame: Frame) -> Result<Bounds> {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for d in &self.data {
            let p = frame.boost(d[0], d[1], d[2]);
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
        let (mut min, mut max) = ([0.0f64; 3], [0.0f64; 3]);
        world.all_reduce_into(&lo[..], &mut min[..], SystemOperation::min());
        world.all_reduce_into(&hi[..], &mut max[..], SystemOperation::max());
        ensure!(min[0].is_finite(), "no particles on any rank");
        Ok(Bounds::from_extent(min, max))
    }

    /// Range boundaries that give each rank about the same total weight.
    fn sample_splitters(&self, world: &SimpleCommunicator) -> Result<Vec<u64>> {
        let ranks = world.size() as usize;
        let mut local: Vec<(u64, u64)> = self.key.iter().copied().zip(self.weight.iter().copied()).collect();
        local.sort_unstable();
        let mut running = 0u64;
        let upto: Vec<u64> = local
            .iter()
            .map(|&(_, w)| {
                running += w.max(1);
                running
            })
            .collect();
        // Every sample must stand for about the same weight, wherever it
        // is. With a fixed count per rank, a first build with all particles
        // on one rank left half the ranks empty.
        let mut total = 0u64;
        world.all_reduce_into(&running, &mut total, SystemOperation::sum());
        ensure!(total > 0, "no particles on any rank");
        let wanted = (SAMPLES_PER_RANK * ranks) as u128 * running as u128;
        let take = (wanted.div_ceil(total as u128) as usize).min(local.len());
        let mine: Vec<u64> = (0..take)
            .map(|i| {
                let at = ((2 * i + 1) as u128 * running as u128 / (2 * take) as u128) as u64;
                local[upto.partition_point(|&u| u <= at).min(local.len() - 1)].0
            })
            .collect();
        let mut samples = gather_all(world, &mine)?;
        samples.sort_unstable();
        ensure!(!samples.is_empty(), "no particles on any rank");
        let mut splitters = vec![0u64; ranks + 1];
        splitters[ranks] = u64::MAX;
        for r in 1..ranks {
            splitters[r] = samples[r * samples.len() / ranks];
        }
        Ok(splitters)
    }

    /// Send every particle to the rank that owns its key, then sort.
    fn migrate(&mut self, world: &SimpleCommunicator) -> Result<()> {
        let ranks = world.size() as usize;
        let mut words: Vec<Vec<u64>> = vec![Vec::new(); ranks];
        let mut values: Vec<Vec<f64>> = vec![Vec::new(); ranks];
        let mut lists: Vec<Vec<u64>> = vec![Vec::new(); ranks];
        for i in 0..self.len() {
            let to = self.owner(self.key[i]);
            words[to].extend([self.id[i], self.key[i], self.leaf_level[i] as u64, self.weight[i]]);
            values[to].extend(self.data[i]);
            lists[to].push(self.prev[i].len() as u64);
            lists[to].extend(&self.prev[i]);
        }
        let words: Vec<u64> = exchange(world, &words)?.into_iter().flatten().collect();
        let values: Vec<f64> = exchange(world, &values)?.into_iter().flatten().collect();
        let lists: Vec<u64> = exchange(world, &lists)?.into_iter().flatten().collect();
        let n = words.len() / 4;
        ensure!(values.len() == n * FIELDS, "particle migration lost data");
        let mut prev = Vec::with_capacity(n);
        let mut at = 0;
        for _ in 0..n {
            let len = lists[at] as usize;
            prev.push(lists[at + 1..at + 1 + len].to_vec());
            at += 1 + len;
        }

        let mut order: Vec<usize> = (0..n).collect();
        order.sort_unstable_by_key(|&i| (words[4 * i + 1], words[4 * i]));
        self.id = order.iter().map(|&i| words[4 * i]).collect();
        self.key = order.iter().map(|&i| words[4 * i + 1]).collect();
        self.leaf_level = order.iter().map(|&i| words[4 * i + 2] as u8).collect();
        self.weight = order.iter().map(|&i| words[4 * i + 3]).collect();
        self.data = order
            .iter()
            .map(|&i| values[FIELDS * i..FIELDS * (i + 1)].try_into().unwrap())
            .collect();
        self.prev = order.iter().map(|&i| std::mem::take(&mut prev[i])).collect();
        Ok(())
    }

    /// `fmmcpp`'s refinement, level by level, with particle counts summed
    /// across ranks. Sets `leaf_level` for every local particle.
    fn refine(&mut self, world: &SimpleCommunicator, q: usize) -> Result<()> {
        let mut level = 2u32;
        let mut examined = self.occupied(level, 0, self.len());
        loop {
            let live = examined.len() as u64;
            let mut any = 0u64;
            world.all_reduce_into(&live, &mut any, SystemOperation::sum());
            if any == 0 {
                return Ok(());
            }
            let counts = self.global_counts(world, level, &examined)?;
            let mut next = Vec::new();
            let mut too_deep = 0u8;
            for &number in &examined {
                let around: u64 = neighbours(level, number).iter().map(|n| counts.get(*n)).sum();
                let (lo, hi) = key_range(level, number);
                let (first, end) = self.slots(lo, hi);
                if around <= q as u64 {
                    self.leaf_level[first..end].fill(level as u8);
                } else if level >= FINEST_LEVEL {
                    too_deep = 1;
                } else {
                    next.extend(self.occupied(level + 1, first, end));
                }
            }
            let mut failed = 0u8;
            world.all_reduce_into(&too_deep, &mut failed, SystemOperation::max());
            if failed != 0 {
                bail!(
                    "FMM tree reached level {level} with more than q={q} particles \
                     around one box; q is too small for this distribution"
                );
            }
            examined = next;
            level += 1;
        }
    }

    /// Distinct level-`level` boxes among local slots `first..end`.
    fn occupied(&self, level: u32, first: usize, end: usize) -> Vec<u64> {
        let mut out = Vec::new();
        let mut at = first;
        while at < end {
            let number = box_number(self.key[at], level);
            out.push(number);
            let hi = key_range(level, number).1;
            at += self.key[at..end].partition_point(|&k| k < hi);
        }
        out
    }

    /// Particle counts, over all ranks, of every box around `examined`.
    fn global_counts(
        &self,
        world: &SimpleCommunicator,
        level: u32,
        examined: &[u64],
    ) -> Result<Counts> {
        if level < DENSE_COUNT_LEVELS {
            let mut local = vec![0u64; 1usize << (3 * level)];
            for &key in &self.key {
                local[box_number(key, level) as usize] += 1;
            }
            let mut global = vec![0u64; local.len()];
            world.all_reduce_into(&local[..], &mut global[..], SystemOperation::sum());
            return Ok(Counts::Dense(global));
        }

        let ranks = world.size() as usize;
        let me = world.rank() as usize;
        let mut wanted: Vec<u64> = examined.iter().flat_map(|&n| neighbours(level, n)).collect();
        wanted.sort_unstable();
        wanted.dedup();

        let mut counts = HashMap::with_capacity_and_hasher(wanted.len(), Default::default());
        let mut asks: Vec<Vec<u64>> = vec![Vec::new(); ranks];
        for &number in &wanted {
            let (lo, hi) = key_range(level, number);
            let (first, end) = self.slots(lo, hi);
            counts.insert(number, (end - first) as u64);
            for rank in self.owner(lo)..=self.owner(hi - 1) {
                if rank != me {
                    asks[rank].push(number);
                }
            }
        }
        let asked = exchange(world, &asks)?;
        let answers: Vec<Vec<u64>> = asked
            .iter()
            .map(|numbers| {
                numbers
                    .iter()
                    .map(|&number| {
                        let (lo, hi) = key_range(level, number);
                        let (first, end) = self.slots(lo, hi);
                        (end - first) as u64
                    })
                    .collect()
            })
            .collect();
        let replies = exchange(world, &answers)?;
        for (numbers, reply) in asks.iter().zip(&replies) {
            ensure!(numbers.len() == reply.len(), "box count reply has the wrong length");
            for (number, count) in numbers.iter().zip(reply) {
                *counts.get_mut(number).unwrap() += count;
            }
        }
        Ok(Counts::Sparse(counts))
    }
}

enum Counts {
    Dense(Vec<u64>),
    Sparse(HashMap<u64, u64>),
}

impl Counts {
    fn get(&self, number: u64) -> u64 {
        match self {
            Counts::Dense(v) => v[number as usize],
            Counts::Sparse(m) => m.get(&number).copied().unwrap_or(0),
        }
    }
}

fn displacements(counts: &[mpi::Count]) -> Result<Vec<mpi::Count>> {
    let mut out = Vec::with_capacity(counts.len());
    let mut total: i64 = 0;
    for &c in counts {
        out.push(mpi::Count::try_from(total).map_err(|_| {
            anyhow::anyhow!("one exchange is over 2^31 values; it must be split")
        })?);
        total += c as i64;
    }
    ensure!(total <= mpi::Count::MAX as i64, "one exchange is over 2^31 values; it must be split");
    Ok(out)
}

/// Send `per_rank[r]` to rank `r`. Returns what each rank sent here.
pub fn exchange<T>(world: &SimpleCommunicator, per_rank: &[Vec<T>]) -> Result<Vec<Vec<T>>>
where
    T: Equivalence + Clone + Default,
{
    let ranks = world.size() as usize;
    ensure!(per_rank.len() == ranks, "exchange needs one buffer per rank");
    let send_counts: Vec<mpi::Count> = per_rank.iter().map(|v| v.len() as mpi::Count).collect();
    let mut recv_counts = vec![0 as mpi::Count; ranks];
    world.all_to_all_into(&send_counts[..], &mut recv_counts[..]);
    let send_displs = displacements(&send_counts)?;
    let recv_displs = displacements(&recv_counts)?;
    // Buffers are never left empty: an empty Vec's pointer is its type's
    // alignment, and for bytes that is 1, which Open MPI reads as
    // MPI_IN_PLACE and rejects. One spare element keeps both on the heap.
    let mut send: Vec<T> = Vec::with_capacity(per_rank.iter().map(|v| v.len()).sum::<usize>() + 1);
    send.extend(per_rank.iter().flatten().cloned());
    let sent = send.len();
    send.push(T::default());
    let total: usize = recv_counts.iter().map(|&c| c as usize).sum();
    let mut recv = vec![T::default(); total + 1];
    {
        let out = Partition::new(&send[..sent], &send_counts[..], &send_displs[..]);
        let mut into = PartitionMut::new(&mut recv[..total], &recv_counts[..], &recv_displs[..]);
        world.all_to_all_varcount_into(&out, &mut into);
    }
    let mut result = Vec::with_capacity(ranks);
    let mut at = 0;
    for &c in &recv_counts {
        result.push(recv[at..at + c as usize].to_vec());
        at += c as usize;
    }
    Ok(result)
}

/// Every rank's `mine`, concatenated in rank order, on every rank.
/// Only for small data such as samples.
pub fn gather_all<T>(world: &SimpleCommunicator, mine: &[T]) -> Result<Vec<T>>
where
    T: Equivalence + Clone + Default,
{
    let ranks = world.size() as usize;
    let count = mine.len() as mpi::Count;
    let mut counts = vec![0 as mpi::Count; ranks];
    world.all_gather_into(&count, &mut counts[..]);
    let displs = displacements(&counts)?;
    let total: usize = counts.iter().map(|&c| c as usize).sum();
    // A spare element keeps the buffers off dangling pointers (see exchange).
    let mut all = vec![T::default(); total + 1];
    let mut own: Vec<T> = Vec::with_capacity(mine.len() + 1);
    own.extend_from_slice(mine);
    own.push(T::default());
    {
        let mut into = PartitionMut::new(&mut all[..total], &counts[..], &displs[..]);
        world.all_gather_varcount_into(&own[..mine.len()], &mut into);
    }
    all.truncate(total);
    Ok(all)
}

/// A variable passed to a `PS*` intrinsic: a real, or a list of reals.
/// Typed variables and untyped fox cells both work.
pub trait PsCell {
    fn ps_scalar(&self) -> Result<f64>;
    fn ps_set_scalar(&mut self, value: f64) -> Result<()>;
    fn ps_values(&self) -> Result<Vec<f64>>;
    fn ps_set_values(&mut self, values: Vec<f64>) -> Result<()>;
    /// An array of untyped cells, such as one that holds expansions.
    fn ps_cells(&mut self) -> Result<&mut Vec<crate::RosyValue>> {
        bail!("expected an array of untyped cells")
    }
    /// The array itself, when the variable is a typed list of reals: kernels
    /// then work on it in place instead of on a copy.
    fn ps_vec_mut(&mut self) -> Option<&mut Vec<f64>> {
        None
    }
    /// The first `count` entries of an array of strings (typed, or untyped
    /// cells holding strings); entries past them may hold anything.
    fn ps_strings(&self, _count: usize) -> Result<Vec<String>> {
        bail!("expected an array of strings")
    }
}

impl PsCell for Vec<String> {
    fn ps_scalar(&self) -> Result<f64> {
        bail!("expected a real, found an array of strings")
    }
    fn ps_set_scalar(&mut self, _value: f64) -> Result<()> {
        bail!("expected a real, found an array of strings")
    }
    fn ps_values(&self) -> Result<Vec<f64>> {
        bail!("expected an array of reals, found an array of strings")
    }
    fn ps_set_values(&mut self, _values: Vec<f64>) -> Result<()> {
        bail!("expected an array of reals, found an array of strings")
    }
    fn ps_strings(&self, count: usize) -> Result<Vec<String>> {
        ensure!(self.len() >= count, "{} strings given, {count} needed", self.len());
        Ok(self[..count].to_vec())
    }
}

impl PsCell for f64 {
    fn ps_scalar(&self) -> Result<f64> {
        Ok(*self)
    }
    fn ps_set_scalar(&mut self, value: f64) -> Result<()> {
        *self = value;
        Ok(())
    }
    fn ps_values(&self) -> Result<Vec<f64>> {
        bail!("expected an array, found a real")
    }
    fn ps_set_values(&mut self, _values: Vec<f64>) -> Result<()> {
        bail!("expected an array, found a real")
    }
}

impl PsCell for Vec<f64> {
    fn ps_vec_mut(&mut self) -> Option<&mut Vec<f64>> {
        Some(self)
    }
    fn ps_scalar(&self) -> Result<f64> {
        bail!("expected a real, found an array")
    }
    fn ps_set_scalar(&mut self, _value: f64) -> Result<()> {
        bail!("expected a real, found an array")
    }
    fn ps_values(&self) -> Result<Vec<f64>> {
        Ok(self.clone())
    }
    fn ps_set_values(&mut self, values: Vec<f64>) -> Result<()> {
        *self = values;
        Ok(())
    }
}

impl PsCell for crate::RosyValue {
    fn ps_scalar(&self) -> Result<f64> {
        match self {
            crate::RosyValue::RE(v) => Ok(*v),
            other => bail!("expected a real, found {other:?}"),
        }
    }
    fn ps_set_scalar(&mut self, value: f64) -> Result<()> {
        *self = crate::RosyValue::RE(value);
        Ok(())
    }
    fn ps_values(&self) -> Result<Vec<f64>> {
        match self {
            crate::RosyValue::VE(v) => Ok(v.clone()),
            crate::RosyValue::RE(v) => Ok(vec![*v]),
            crate::RosyValue::Arr(cells) => cells.iter().map(|c| c.ps_scalar()).collect(),
            other => bail!("expected an array of reals, found {other:?}"),
        }
    }
    fn ps_set_values(&mut self, values: Vec<f64>) -> Result<()> {
        *self = crate::RosyValue::VE(values);
        Ok(())
    }
}

impl PsCell for Vec<crate::RosyValue> {
    fn ps_scalar(&self) -> Result<f64> {
        bail!("expected a real, found an array")
    }
    fn ps_set_scalar(&mut self, _value: f64) -> Result<()> {
        bail!("expected a real, found an array")
    }
    fn ps_values(&self) -> Result<Vec<f64>> {
        self.iter().map(|c| c.ps_scalar()).collect()
    }
    fn ps_set_values(&mut self, values: Vec<f64>) -> Result<()> {
        *self = values.into_iter().map(crate::RosyValue::RE).collect();
        Ok(())
    }
    fn ps_cells(&mut self) -> Result<&mut Vec<crate::RosyValue>> {
        Ok(self)
    }
    fn ps_strings(&self, count: usize) -> Result<Vec<String>> {
        ensure!(self.len() >= count, "{} cells given, {count} needed", self.len());
        self[..count]
            .iter()
            .map(|c| match c {
                crate::RosyValue::ST(s) => Ok(s.clone()),
                other => bail!("expected a string, found {}", other.kind_name()),
            })
            .collect()
    }
}

/// A C function a program supplies: how many arrays, the arrays, their lengths.
pub type KernelFn = unsafe extern "C" fn(i64, *mut *mut f64, *const i64);

/// The DA monomials in the order kernels see them: by total degree, then
/// by exponents, higher powers of earlier variables first (for three
/// variables at degree 2: x1², x1x2, x1x3, x2², x2x3, x3²). Entry k is the
/// runtime's flat index of the k-th monomial.
fn kernel_da_order() -> Result<Vec<usize>> {
    let rt = crate::taylor::get_runtime()?;
    let nv = rt.config.num_vars;
    let mut order: Vec<usize> = (0..rt.num_monomials).collect();
    order.sort_by(|&a, &b| {
        let (ma, mb) = (&rt.monomial_list[a], &rt.monomial_list[b]);
        ma.total_order.cmp(&mb.total_order).then_with(|| mb.exponents[..nv].cmp(&ma.exponents[..nv]))
    });
    Ok(order)
}

/// Call a program's C kernel. `kinds` has one letter per argument (missing
/// letters mean `r`):
/// - `r`: the cell goes over as an array of doubles (a real is an array of
///   one).
/// - `d`: an array of untyped cells holding DA values. Each cell goes over
///   as a block of 1 + M doubles, M the number of DA monomials: a flag (1
///   if the cell holds a DA, else 0), then its coefficients in
///   [`kernel_da_order`]. A block the kernel leaves with flag 1 becomes a
///   DA (coefficients at or below the DA epsilon are dropped); a block left
///   with flag 0 leaves its cell as it was.
///
/// What the kernel changed is written back. The kernel must stay inside the
/// lengths it is given.
pub fn kernel_call(kernel: KernelFn, kinds: &str, cells: &mut [&mut dyn PsCell]) -> Result<()> {
    let kinds: Vec<u8> = kinds.bytes().collect();
    let kind = |at: usize| kinds.get(at).copied().unwrap_or(b'r');
    let da_order = if kinds.contains(&b'd') { Some(kernel_da_order()?) } else { None };
    let block = da_order.as_ref().map_or(0, |o| o.len() + 1);
    let mut scalar = Vec::with_capacity(cells.len());
    let mut before = Vec::with_capacity(cells.len());
    // Typed real arrays go over in place: no copy in, no comparison, no copy
    // back. Everything else goes over as a copy and changes are written back.
    let mut in_place: Vec<Option<(*mut f64, usize)>> = Vec::with_capacity(cells.len());
    for (at, cell) in cells.iter_mut().enumerate() {
        if kind(at) == b'r' {
            if let Some(v) = cell.ps_vec_mut() {
                in_place.push(Some((v.as_mut_ptr(), v.len())));
                scalar.push(false);
                before.push(Vec::new());
                continue;
            }
        }
        in_place.push(None);
        match kind(at) {
            b'd' => {
                let order = da_order.as_ref().expect("DA order built when a d argument is present");
                let list = cell.ps_cells().map_err(|e| e.context(format!("kernel argument {} (kind d)", at + 1)))?;
                let mut flat = vec![0.0; list.len() * block];
                for (c, value) in list.iter().enumerate() {
                    if let crate::RosyValue::DA(da) = value {
                        let out = &mut flat[c * block..(c + 1) * block];
                        out[0] = 1.0;
                        for (k, &flat_index) in order.iter().enumerate() {
                            out[1 + k] = da.coeffs[flat_index];
                        }
                    }
                }
                scalar.push(false);
                before.push(flat);
            }
            b'r' => match cell.ps_scalar() {
                Ok(v) => {
                    scalar.push(true);
                    before.push(vec![v]);
                }
                Err(_) => {
                    scalar.push(false);
                    before.push(cell.ps_values().map_err(|e| e.context(format!("kernel argument {}", at + 1)))?);
                }
            },
            other => bail!("kernel argument {}: unknown kind '{}' (expected r or d)", at + 1, other as char),
        }
    }
    let mut after = before.clone();
    let lengths: Vec<i64> = after
        .iter()
        .zip(&in_place)
        .map(|(v, place)| place.map_or(v.len(), |(_, len)| len) as i64)
        .collect();
    let mut pointers: Vec<*mut f64> = after
        .iter_mut()
        .zip(&in_place)
        .map(|(v, place)| place.map_or(v.as_mut_ptr(), |(ptr, _)| ptr))
        .collect();
    // SAFETY: each pointer covers the length passed with it. Copies live in
    // `after`; in-place pointers point into the variables' own vectors, which
    // nothing touches until the call returns. A variable cannot be passed
    // twice (each argument is a separate &mut).
    unsafe { kernel(pointers.len() as i64, pointers.as_mut_ptr(), lengths.as_ptr()) };
    for (at, cell) in cells.iter_mut().enumerate() {
        if in_place[at].is_some() {
            continue;
        }
        let same = before[at].iter().zip(&after[at]).all(|(a, b)| a.to_bits() == b.to_bits());
        if same {
            continue;
        }
        if kind(at) == b'd' {
            let order = da_order.as_ref().expect("DA order built when a d argument is present");
            let epsilon = crate::taylor::get_runtime()?.config.epsilon;
            let list = cell.ps_cells()?;
            for (c, value) in list.iter_mut().enumerate() {
                let (old, new) = (&before[at][c * block..(c + 1) * block], &after[at][c * block..(c + 1) * block]);
                if new[0] != 1.0 || old.iter().zip(new).all(|(a, b)| a.to_bits() == b.to_bits()) {
                    continue;
                }
                let mut da = crate::DA::zero();
                for (k, &flat_index) in order.iter().enumerate() {
                    let v = new[1 + k];
                    if v.abs() > epsilon {
                        da.coeffs[flat_index] = v;
                        da.nonzero.push(flat_index as u32);
                    }
                }
                *value = crate::RosyValue::DA(da);
            }
        } else if scalar[at] {
            cell.ps_set_scalar(after[at][0])?;
        } else {
            cell.ps_set_values(std::mem::take(&mut after[at]))?;
        }
    }
    Ok(())
}

/// Argument count of each `PS*` intrinsic. The compiler checks against it.
pub const PS_INTRINSICS: &[(&str, usize)] = &[
    ("PSLOAD", 10),
    ("PSBUILD", 5),
    ("PSGET", 11),
    ("PSLISTS", 8),
    ("PSPUT", 7),
    ("PSSCALED", 3),
    ("PSSIZES", 3),
    ("PSLEAFLOC", 1),
    ("PSUP", 3),
    ("PSDOWN", 3),
    ("PSMSYNC", 2),
    ("PSLSYNC", 2),
    ("PSHALO", 0),
    ("PSCOST", 2),
    ("PSBOUNDS", 4),
    ("PSPREV", 2),
    ("PSPOS", 2),
    ("PSSUM", 3),
    ("PSMAX", 3),
    ("PSALL", 10),
    ("PSSLICE", 11),
    ("PSWRITE", 3),
    ("PSREMOVE", 6),
];

/// The parts of `PSBUILD` timed separately in the stats report.
/// After them, the parts of `PSMSYNC` and `PSLSYNC` together (the wait is
/// measured only when `ROSY_MPI_STATS` is set).
pub const BUILD_PARTS: &[&str] = &[
    "  build: sort and refine",
    "  build: pass plan",
    "  build: halo",
    "  build: earlier neighbours",
    "  sync: waiting for slowest",
    "  sync: encode",
    "  sync: exchange",
    "  sync: decode",
];

/// Whether `ROSY_MPI_STATS` is set (read once).
fn stats_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("ROSY_MPI_STATS").is_some())
}

impl crate::mpi::RosyMPIContext {
    /// Run one `PS*` intrinsic. Every rank must make the same call.
    ///
    /// - `PSLOAD x y z px py pz f n e count`: give the store the first
    ///   `count` entries of this rank's arrays. Particle numbers continue
    ///   from the last load, lower ranks first.
    /// - `PSBUILD q wx wy wz relg`: recompute keys from positions boosted
    ///   into the beam's rest frame exactly as PHAD's `SC_A` does it (`W =
    ///   gamma*beta` of the common velocity, `relg = gamma`, both as the
    ///   program computed them), move particles to their owners, rebuild
    ///   leaves, fetch the halo. Stored positions stay in the lab frame.
    /// - `PSGET x y z px py pz f n e ids nown`: fill the arrays with this
    ///   rank's particles, then its halo copies. `ids` are particle
    ///   numbers from 1. `nown` is how many entries are owned.
    /// - `PSLISTS snumbs start_snumbs nnumbs start_nnumbs cx cy cz nleaves`:
    ///   per-leaf source and neighbour lists as positions (from 1) into the
    ///   `PSGET` arrays, with the offsets of each leaf's run and the leaf
    ///   centres in the unit cube. Sources are in particle-number order.
    ///   Neighbours come box by box, each box in particle-number order.
    /// - `PSPUT x y z px py pz e`: store the first `nown` entries back.
    /// - `PSSCALED sx sy sz`: positions in the unit cube the tree was built
    ///   in (boosted, then scaled), for the same entries as `PSGET`.
    /// - `PSSIZES nmult nloc depth`: how many multipoles and local
    ///   expansions this rank holds, copies included, and the level count.
    /// - `PSLEAFLOC index`: for each leaf, in `PSLISTS` order, the position
    ///   of its local expansion.
    /// - `PSUP level list count` and `PSDOWN level list count`: the work
    ///   lists of [`ParticleStore::upward_list`] and
    ///   [`ParticleStore::downward_list`].
    /// - `PSMSYNC cells level` and `PSLSYNC cells level`: exchange this
    ///   level's multipoles or local expansions between ranks.
    /// - `PSHALO`: refresh the halo copies after `PSPUT`.
    /// - `PSCOST leaf seconds`: charge `seconds` of work to owned leaf
    ///   `leaf` (its position in `PSLISTS` order, from 1). The next
    ///   `PSBUILD` balances the ranks by these costs plus the time each
    ///   rank spent between `PSUP`/`PSDOWN` and the following sync. A
    ///   program that never calls it is balanced by an estimate. It is
    ///   local: ranks need not call it the same number of times.
    /// - `PSBOUNDS xmin ymin zmin dzero`: the cube of the last `PSBUILD`,
    ///   in the rest frame.
    /// - `PSPREV ids start`: for each owned particle, the particle numbers
    ///   (from 1) around it before the last `PSBUILD`, with offsets.
    /// - `PSPOS ids positions`: local position of each particle number, 0
    ///   when this rank holds no copy.
    /// - `PSSUM values count total`: the sum over all ranks of the first
    ///   `count` values, correctly rounded, so it does not depend on the
    ///   rank count. With `ROSY_PS_ORDERED_SUM` set it instead adds one
    ///   value per owned particle in particle-number order, as a
    ///   one-process loop would; that gathers everything and is for
    ///   comparisons only.
    /// - `PSMAX values count max`: the largest of the first `count` values
    ///   over all ranks, on every rank. Exact, so it does not depend on
    ///   the rank count. `count` must be at least 1 on every rank.
    /// - `PSALL x y z px py pz f n e count`: every particle in number order
    ///   on the first rank, nothing elsewhere. For output only.
    /// - `PSSLICE x y z px py pz f n e first count`: this rank's share of the
    ///   particles in number order: particle numbers `first ..
    ///   first+count-1`, with rank r getting the r-th of `ranks` equal runs
    ///   of the numbers loaded. Lets every rank format and write its own
    ///   part of an output file.
    /// - `PSREMOVE keep info k removed nremoved ncool`: drop the owned
    ///   particles whose `keep` flag (one per owned particle, in `PSGET`
    ///   order) is 0. `info` holds `k` reals per owned particle, particle
    ///   after particle (`k` may be 0). Every rank then holds `removed`: one
    ///   row `old_number info(1..k)` per removed particle, all ranks' rows,
    ///   sorted by old number, and `nremoved`, their count. Survivors are
    ///   renumbered `1..` keeping their order, `old - (removed numbers below
    ///   it)`, which is a one-process program's in-place compaction, and
    ///   `ncool` (the trailing particles, cooling electrons in PHAD) drops by
    ///   the removed numbers among the last `ncool`. No particle moves. When
    ///   something was removed the store is as after `PSLOAD`: call
    ///   `PSBUILD` before using the tree again (a first build: earlier
    ///   neighbour lists start empty, as PHAD's restarted force cycle
    ///   expects). When nothing was removed nothing changes.
    /// - `PSWRITE unit lines count`: the first `count` strings of every
    ///   rank's `lines`, lower ranks first, written as lines to file `unit`
    ///   by the first rank (only it needs the file open), in one write.
    ///   With `PSSLICE`, every rank formats its own run of particles and
    ///   the file comes out in particle order.
    pub fn ps_call(&mut self, name: &str, args: &mut [&mut dyn PsCell]) -> Result<()> {
        let started = std::time::Instant::now();
        let result = self.ps_call_timed(name, args);
        let slots = PS_INTRINSICS.len() + BUILD_PARTS.len();
        if self.pstore.call_seconds.len() != slots {
            self.pstore.call_seconds.resize(slots, 0.0);
        }
        if let Some(at) = PS_INTRINSICS.iter().position(|(n, _)| *n == name) {
            self.pstore.call_seconds[at] += started.elapsed().as_secs_f64();
        }
        result
    }

    fn ps_call_timed(&mut self, name: &str, args: &mut [&mut dyn PsCell]) -> Result<()> {
        let expected = PS_INTRINSICS.iter().find(|(n, _)| *n == name).map(|(_, c)| *c);
        ensure!(expected == Some(args.len()), "{name}: wrong number of arguments");
        let store = &mut self.pstore;
        let world = &self.world;
        match name {
            "PSLOAD" => {
                let count = args[FIELDS].ps_scalar()? as usize;
                let columns: Vec<Vec<f64>> =
                    args[..FIELDS].iter().map(|a| a.ps_values()).collect::<Result<_>>()?;
                let short = columns.iter().any(|c| c.len() < count);
                let counts = gather_all(world, &[if short { u64::MAX } else { count as u64 }])?;
                ensure!(
                    !counts.contains(&u64::MAX),
                    "PSLOAD: an array is shorter than the particle count"
                );
                let me = world.rank() as usize;
                let base = store.loaded + counts[..me].iter().sum::<u64>();
                store.loaded += counts.iter().sum::<u64>();
                for i in 0..count {
                    let mut data = [0.0; FIELDS];
                    for (d, column) in data.iter_mut().zip(&columns) {
                        *d = column[i];
                    }
                    store.push(base + i as u64, data);
                }
            }
            "PSBUILD" => {
                let q = args[0].ps_scalar()? as usize;
                let frame = Frame {
                    w: [args[1].ps_scalar()?, args[2].ps_scalar()?, args[3].ps_scalar()?],
                    relg: args[4].ps_scalar()?,
                };
                // The first build has no work estimates yet, so it balances
                // by count. Build once to get them, then again to use them.
                let passes = if store.leaves.is_empty() { 2 } else { 1 };
                store.apply_costs(world)?;
                let mut parts = [0.0f64; 4];
                let mut mark = std::time::Instant::now();
                let mut lap = |part: usize| {
                    parts[part] += mark.elapsed().as_secs_f64();
                    mark = std::time::Instant::now();
                };
                for _ in 0..passes {
                    store.rebuild(world, q, frame)?;
                    lap(0);
                    store.build_plan(world)?;
                    lap(1);
                    store.exchange_halo(world)?;
                    lap(2);
                }
                store.fetch_previous(world)?;
                lap(3);
                store.call_seconds.resize(PS_INTRINSICS.len() + BUILD_PARTS.len(), 0.0);
                for (slot, part) in store.call_seconds[PS_INTRINSICS.len()..].iter_mut().zip(parts) {
                    *slot += part;
                }
                store.leaf_cost = vec![0.0; store.leaves.len()];
            }
            "PSCOST" => {
                let leaf = args[0].ps_scalar()?;
                let seconds = args[1].ps_scalar()?;
                ensure!(
                    leaf >= 1.0 && leaf as usize <= store.leaf_cost.len(),
                    "PSCOST: leaf {leaf} is not one of this rank's {} leaves",
                    store.leaf_cost.len()
                );
                store.leaf_cost[leaf as usize - 1] += seconds.max(0.0);
            }
            "PSGET" => {
                let all = || store.data.iter().chain(&store.halo_data).chain(&store.far_data);
                for (field, arg) in args[..FIELDS].iter_mut().enumerate() {
                    arg.ps_set_values(all().map(|d| d[field]).collect())?;
                }
                let ids = store.id.iter().chain(&store.halo_id).chain(&store.far_id);
                args[FIELDS].ps_set_values(ids.map(|&id| (id + 1) as f64).collect())?;
                args[FIELDS + 1].ps_set_scalar(store.len() as f64)?;
            }
            "PSLISTS" => {
                let (mut own, mut own_start) = (Vec::new(), vec![1.0]);
                let (mut near, mut near_start) = (Vec::new(), vec![1.0]);
                let mut centre: [Vec<f64>; 3] = Default::default();
                for leaf in &store.leaves {
                    let mine = store.box_positions(leaf.level, leaf.number);
                    own.extend(mine.iter().map(|&p| (p + 1) as f64));
                    own_start.push((own.len() + 1) as f64);
                    // Box by box, as `fmmcpp` writes them. PHAD sorts each
                    // leaf's list itself.
                    let around = store.neighbour_positions(leaf);
                    near.extend(around.iter().map(|&p| (p + 1) as f64));
                    near_start.push((near.len() + 1) as f64);
                    let c = crate::fmmtree::box_centre(leaf.level, leaf.number);
                    for a in 0..3 {
                        centre[a].push(c[a]);
                    }
                }
                let [cx, cy, cz] = centre;
                let leaves = store.leaves.len() as f64;
                for (arg, values) in args[..7].iter_mut().zip([own, own_start, near, near_start, cx, cy, cz]) {
                    arg.ps_set_values(values)?;
                }
                args[7].ps_set_scalar(leaves)?;
            }
            "PSPUT" => {
                for (&field, arg) in PUT_FIELDS.iter().zip(args.iter()) {
                    let values = arg.ps_values()?;
                    ensure!(values.len() >= store.len(), "PSPUT: array is shorter than the owned particles");
                    for (d, v) in store.data.iter_mut().zip(values) {
                        d[field] = v;
                    }
                }
            }
            "PSSCALED" => {
                let bounds = store.bounds.ok_or_else(|| anyhow::anyhow!("PSSCALED before PSBUILD"))?;
                let frame = store.frame;
                let all = || store.data.iter().chain(&store.halo_data).chain(&store.far_data);
                for (axis, arg) in args.iter_mut().enumerate() {
                    arg.ps_set_values(
                        all().map(|d| bounds.scale(frame.boost(d[0], d[1], d[2]))[axis]).collect(),
                    )?;
                }
            }
            "PSSIZES" => {
                args[0].ps_set_scalar(store.plan.multipole_index.len() as f64)?;
                args[1].ps_set_scalar(store.plan.local_index.len() as f64)?;
                args[2].ps_set_scalar(store.plan.depth as f64)?;
            }
            "PSLEAFLOC" => {
                let index = store
                    .leaves
                    .iter()
                    .map(|leaf| (store.plan.local_index[&(leaf.level, leaf.number)] + 1) as f64)
                    .collect();
                args[0].ps_set_values(index)?;
            }
            "PSUP" | "PSDOWN" => {
                let level = args[0].ps_scalar()? as u32;
                let (list, count) = if name == "PSUP" {
                    store.upward_list(level)
                } else {
                    store.downward_list(level)
                };
                args[1].ps_set_values(list)?;
                args[2].ps_set_scalar(count as f64)?;
                store.pass_mark = Some(std::time::Instant::now());
            }
            "PSMSYNC" => {
                if let Some(mark) = store.pass_mark.take() {
                    store.pass_seconds += mark.elapsed().as_secs_f64();
                }
                let level = args[1].ps_scalar()? as u32;
                let plan = &store.plan;
                let parts = store.sync_cells(world, args[0].ps_cells()?, level, &plan.multipole_users, &plan.ghost_multipoles, &plan.multipole_index)?;
                store.add_sync_parts(parts);
            }
            "PSLSYNC" => {
                if let Some(mark) = store.pass_mark.take() {
                    store.pass_seconds += mark.elapsed().as_secs_f64();
                }
                let level = args[1].ps_scalar()? as u32;
                let plan = &store.plan;
                let parts = store.sync_cells(world, args[0].ps_cells()?, level, &plan.local_users, &plan.ghost_locals, &plan.local_index)?;
                store.add_sync_parts(parts);
            }
            "PSHALO" => store.exchange_halo(world)?,
            "PSBOUNDS" => {
                let bounds = store.bounds.ok_or_else(|| anyhow::anyhow!("PSBOUNDS before PSBUILD"))?;
                for (arg, value) in args.iter_mut().zip([bounds.min[0], bounds.min[1], bounds.min[2], bounds.dzero]) {
                    arg.ps_set_scalar(value)?;
                }
            }
            "PSPREV" => {
                let mut ids = Vec::new();
                let mut start = vec![1.0];
                for list in &store.prev {
                    ids.extend(list.iter().map(|&id| (id + 1) as f64));
                    start.push((ids.len() + 1) as f64);
                }
                args[0].ps_set_values(ids)?;
                args[1].ps_set_values(start)?;
            }
            "PSPOS" => {
                let positions = args[0]
                    .ps_values()?
                    .iter()
                    .map(|&id| match store.position_of.get(&((id - 1.0) as u64)) {
                        Some(&p) if id >= 1.0 => (p + 1) as f64,
                        _ => 0.0,
                    })
                    .collect();
                args[1].ps_set_values(positions)?;
            }
            "PSSUM" => {
                let count = args[1].ps_scalar()? as usize;
                let values = args[0].ps_values()?;
                ensure!(values.len() >= count, "PSSUM: array is shorter than the count");
                let total = if std::env::var_os("ROSY_PS_ORDERED_SUM").is_some() {
                    ensure!(count == store.len(), "PSSUM in ordered mode sums one value per owned particle");
                    let ids = gather_all(world, &store.id)?;
                    let all = gather_all(world, &values[..count])?;
                    let mut order: Vec<usize> = (0..ids.len()).collect();
                    order.sort_unstable_by_key(|&i| ids[i]);
                    order.iter().fold(0.0, |sum, &i| sum + all[i])
                } else {
                    let partials = gather_all(world, &exact_partials(&values[..count]))?;
                    round_partials(exact_partials(&partials))
                };
                args[2].ps_set_scalar(total)?;
            }
            "PSMAX" => {
                let count = args[1].ps_scalar()? as usize;
                let values = args[0].ps_values()?;
                ensure!(count >= 1, "PSMAX: count must be at least 1");
                ensure!(values.len() >= count, "PSMAX: array is shorter than the count");
                let mine = values[..count].iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let mut all = 0.0f64;
                world.all_reduce_into(&mine, &mut all, SystemOperation::max());
                args[2].ps_set_scalar(all)?;
            }
            "PSALL" => {
                let ranks = world.size() as usize;
                let mut ids: Vec<Vec<u64>> = vec![Vec::new(); ranks];
                let mut values: Vec<Vec<f64>> = vec![Vec::new(); ranks];
                ids[0] = store.id.clone();
                values[0] = store.data.iter().flatten().copied().collect();
                let ids: Vec<u64> = exchange(world, &ids)?.into_iter().flatten().collect();
                let values: Vec<f64> = exchange(world, &values)?.into_iter().flatten().collect();
                let mut order: Vec<usize> = (0..ids.len()).collect();
                order.sort_unstable_by_key(|&i| ids[i]);
                for (field, arg) in args[..FIELDS].iter_mut().enumerate() {
                    arg.ps_set_values(order.iter().map(|&i| values[FIELDS * i + field]).collect())?;
                }
                args[FIELDS].ps_set_scalar(ids.len() as f64)?;
            }
            "PSSLICE" => {
                let ranks = world.size() as usize;
                let total = store.loaded;
                let start = |r: usize| (total as u128 * r as u128 / ranks as u128) as u64;
                let starts: Vec<u64> = (0..=ranks).map(start).collect();
                let mut ids: Vec<Vec<u64>> = vec![Vec::new(); ranks];
                let mut values: Vec<Vec<f64>> = vec![Vec::new(); ranks];
                for (&id, d) in store.id.iter().zip(&store.data) {
                    let to = starts.partition_point(|&s| s <= id) - 1;
                    ids[to].push(id);
                    values[to].extend_from_slice(d);
                }
                let ids: Vec<u64> = exchange(world, &ids)?.into_iter().flatten().collect();
                let values: Vec<f64> = exchange(world, &values)?.into_iter().flatten().collect();
                let mut order: Vec<usize> = (0..ids.len()).collect();
                order.sort_unstable_by_key(|&i| ids[i]);
                let me = world.rank() as usize;
                // Particles are never dropped, so the run is complete.
                ensure!(
                    ids.len() as u64 == starts[me + 1] - starts[me],
                    "PSSLICE: {} particles arrived for a run of {}",
                    ids.len(),
                    starts[me + 1] - starts[me]
                );
                for (field, arg) in args[..FIELDS].iter_mut().enumerate() {
                    arg.ps_set_values(order.iter().map(|&i| values[FIELDS * i + field]).collect())?;
                }
                args[FIELDS].ps_set_scalar((starts[me] + 1) as f64)?;
                args[FIELDS + 1].ps_set_scalar(ids.len() as f64)?;
            }
            "PSREMOVE" => {
                let n = store.len();
                let keep = args[0].ps_values()?;
                ensure!(keep.len() >= n, "PSREMOVE: keep is shorter than the owned particles");
                let k = args[2].ps_scalar()?;
                ensure!(k >= 0.0 && k.fract() == 0.0, "PSREMOVE: k must be a whole number, not {k}");
                let k = k as usize;
                let info = if k > 0 { args[1].ps_values()? } else { Vec::new() };
                ensure!(info.len() >= n * k, "PSREMOVE: info is shorter than k values per owned particle");
                let ncool = args[5].ps_scalar()?;
                let total = store.loaded;
                // This rank's rows, then everyone's, by old number.
                let mut mine = Vec::new();
                for slot in 0..n {
                    if keep[slot] == 0.0 {
                        mine.push((store.id[slot] + 1) as f64);
                        mine.extend_from_slice(&info[slot * k..slot * k + k]);
                    }
                }
                let all = gather_all(world, &mine)?;
                let mut rows: Vec<&[f64]> = all.chunks_exact(1 + k).collect();
                rows.sort_unstable_by(|a, b| a[0].total_cmp(&b[0]));
                let gone: Vec<u64> = rows.iter().map(|r| r[0] as u64 - 1).collect();
                let cool_from = total.saturating_sub(ncool.max(0.0) as u64);
                let cool_gone = gone.iter().filter(|&&id| id >= cool_from).count() as f64;
                args[3].ps_set_values(rows.concat())?;
                args[4].ps_set_scalar(gone.len() as f64)?;
                args[5].ps_set_scalar(ncool - cool_gone)?;
                if gone.is_empty() {
                    return Ok(());
                }
                // Drop this rank's flagged particles, keeping the order.
                let kept: Vec<bool> = keep[..n].iter().map(|&f| f != 0.0).collect();
                store.id.retain_by(&kept);
                store.key.retain_by(&kept);
                store.data.retain_by(&kept);
                store.leaf_level.retain_by(&kept);
                store.weight.retain_by(&kept);
                // Renumber as the serial compaction does.
                for id in &mut store.id {
                    *id -= gone.partition_point(|&g| g < *id) as u64;
                }
                store.loaded = total - gone.len() as u64;
                // As after PSLOAD: the tree and everything built on it is gone.
                store.prev = vec![Vec::new(); store.id.len()];
                store.leaves.clear();
                store.leaf_cost.clear();
                store.pass_seconds = 0.0;
                store.pass_mark = None;
                store.plan = Plan::default();
                store.bounds = None;
                store.halo_id.clear();
                store.halo_key.clear();
                store.halo_data.clear();
                store.far_id.clear();
                store.far_data.clear();
                store.position_of.clear();
                store.directory.clear();
            }
            "PSWRITE" => {
                let unit = args[0].ps_scalar()?;
                let count = args[2].ps_scalar()? as usize;
                let lines = args[1].ps_strings(count)?;
                let mut bytes = Vec::with_capacity(lines.iter().map(|l| l.len() + 1).sum());
                for line in &lines {
                    bytes.extend_from_slice(line.as_bytes());
                    bytes.push(b'\n');
                }
                let ranks = world.size() as usize;
                let mut outgoing: Vec<Vec<u8>> = vec![Vec::new(); ranks];
                outgoing[0] = bytes;
                let received = exchange(world, &outgoing)?;
                if world.rank() == 0 {
                    let all: Vec<u8> = received.into_iter().flatten().collect();
                    crate::core::file_io::rosy_write_bytes_to_unit(unit as u64, &all)?;
                }
            }
            other => bail!("unknown particle-store intrinsic {other}"),
        }
        Ok(())
    }
}

/// Keep the entries of a per-particle column whose flag is set.
trait RetainBy {
    fn retain_by(&mut self, kept: &[bool]);
}

impl<T> RetainBy for Vec<T> {
    fn retain_by(&mut self, kept: &[bool]) {
        let mut at = 0;
        self.retain(|_| {
            at += 1;
            kept[at - 1]
        });
    }
}

/// Non-overlapping partial sums whose exact total is the exact sum of
/// `values` (Shewchuk's algorithm, as in Python's `math.fsum`).
fn exact_partials(values: &[f64]) -> Vec<f64> {
    let mut partials: Vec<f64> = Vec::new();
    for &value in values {
        let mut x = value;
        let mut kept = 0;
        for j in 0..partials.len() {
            let mut y = partials[j];
            if x.abs() < y.abs() {
                std::mem::swap(&mut x, &mut y);
            }
            let hi = x + y;
            let lo = y - (hi - x);
            if lo != 0.0 {
                partials[kept] = lo;
                kept += 1;
            }
            x = hi;
        }
        partials.truncate(kept);
        partials.push(x);
    }
    partials
}

/// The correctly rounded total of partials from [`exact_partials`].
fn round_partials(mut partials: Vec<f64>) -> f64 {
    let Some(mut hi) = partials.pop() else {
        return 0.0;
    };
    let mut lo = 0.0;
    while let Some(y) = partials.pop() {
        let x = hi;
        hi = x + y;
        lo = y - (hi - x);
        if lo != 0.0 {
            break;
        }
    }
    // Round half to even when the next partial pushes past the midpoint.
    if let Some(&next) = partials.last() {
        if (lo < 0.0 && next < 0.0) || (lo > 0.0 && next > 0.0) {
            let y = lo * 2.0;
            let x = hi + y;
            if y == x - hi {
                hi = x;
            }
        }
    }
    hi
}

#[cfg(test)]
mod candidate_tests {
    use super::ParticleStore;
    use crate::fmmtree::neighbours;

    /// The plain definition, kept to check the coordinate version against.
    fn by_definition(level: u32, number: u64) -> Vec<u64> {
        let touching = neighbours(level, number);
        let mut out = Vec::new();
        for near_parent in neighbours(level - 1, number >> 3) {
            for child in near_parent * 8..near_parent * 8 + 8 {
                if !touching.contains(&child) {
                    out.push(child);
                }
            }
        }
        out
    }

    #[test]
    fn candidates_match_the_definition_in_order() {
        for level in 2..=7u32 {
            let boxes = 1u64 << (3 * level);
            let step = (boxes / 997).max(1);
            let mut number = 0;
            while number < boxes {
                assert_eq!(ParticleStore::interaction_candidates(level, number), by_definition(level, number), "level {level} box {number}");
                number += step;
            }
            // the corners and edges of the domain
            for number in [0, boxes - 1, boxes / 2, (boxes / 8) * 3 + 5] {
                assert_eq!(ParticleStore::interaction_candidates(level, number), by_definition(level, number), "level {level} box {number}");
            }
        }
    }
}
