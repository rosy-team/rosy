//! Particles spread across MPI ranks by Morton key.
//!
//! Each rank owns one contiguous key range and keeps its particles sorted
//! by `(key, id)`. No rank holds or loops over all particles. The leaf set
//! is the one [`crate::fmmtree::LeafTable`] builds on a single process, for
//! any rank count. See `docs/distributed-storage.md`.

use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::{Result, bail, ensure};
use mpi::collective::SystemOperation;
use mpi::datatype::{Partition, PartitionMut};
use mpi::topology::SimpleCommunicator;
use mpi::traits::*;

use crate::fmmtree::{
    Bounds, FINEST_LEVEL, Leaf, box_centre, box_number, key_range, morton_key, neighbours,
};

/// Values carried per particle: x y z px py pz f n.
pub const FIELDS: usize = 8;
/// Keys offered per rank, on average, when the ranges are chosen. A rank
/// offers in proportion to the particles it holds.
const SAMPLES_PER_RANK: usize = 16;
/// Per-particle weight of the upward and downward passes, on the scale
/// where a leaf's near-field work is its neighbourhood size squared.
const PASS_WEIGHT: u64 = 400;
/// Levels at or above this many boxes are counted by query, not by a
/// dense histogram: 8^6 boxes is 2 MB per rank.
const DENSE_COUNT_LEVELS: u32 = 6;

/// One rank's share of the particles.
#[derive(Debug, Default)]
pub struct ParticleStore {
    /// Original particle number. It never changes.
    pub id: Vec<u64>,
    pub key: Vec<u64>,
    pub data: Vec<[f64; FIELDS]>,
    /// Level of the leaf each particle sits in.
    pub leaf_level: Vec<u8>,
    /// Estimated work each particle costs per step, from its last leaf.
    /// Ranks are balanced by this, not by particle count.
    pub weight: Vec<u64>,
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
    /// The z multiplier of the last rebuild.
    pub z_scale: f64,
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
    /// `z_scale` multiplies z before the key is taken; PHAD passes `relg`.
    /// `q` is the most particles allowed around one leaf.
    pub fn rebuild(&mut self, world: &SimpleCommunicator, q: usize, z_scale: f64) -> Result<()> {
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
        let bounds = self.global_bounds(world, z_scale)?;
        self.bounds = Some(bounds);
        self.z_scale = z_scale;
        for (key, d) in self.key.iter_mut().zip(&self.data) {
            *key = morton_key(bounds.scale([d[0], d[1], d[2] * z_scale]));
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

    /// Children of the parent's neighbours that do not touch the box.
    fn interaction_candidates(level: u32, number: u64) -> Vec<u64> {
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
        let mut seen = HashSet::new();
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
        let mut wanted = HashSet::new();
        for (level, boxes) in self.plan.targets.iter().enumerate() {
            for &number in boxes {
                for c in Self::interaction_candidates(level as u32, number) {
                    wanted.insert((level as u32, c));
                }
            }
        }
        let wanted: Vec<(u32, u64)> = wanted.into_iter().collect();
        self.count_boxes(world, &wanted)?;

        // Multipoles: tell each owner which of its boxes are needed.
        let mut multipoles = vec![BTreeSet::new(); depth];
        let mut requests: Vec<Vec<u64>> = vec![Vec::new(); ranks];
        for (level, boxes) in self.plan.targets.iter().enumerate() {
            let level = level as u32;
            for &number in boxes {
                for c in self.interaction_list(level, number) {
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
        let mut asked = HashSet::new();
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
        for &number in boxes {
            out.push((self.plan.local_index[&(level, number)] + 1) as f64);
            out.extend(box_centre(level, number));
            if level > 2 {
                out.push((self.plan.local_index[&(level - 1, number >> 3)] + 1) as f64);
                out.extend(box_centre(level - 1, number >> 3));
            } else {
                out.extend([0.0; 4]);
            }
            let list = self.interaction_list(level, number);
            out.push(list.len() as f64);
            for other in list {
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
    ) -> Result<()> {
        let ranks = world.size() as usize;
        ensure!(cells.len() >= index.len(), "expansion array holds {} cells but {} are needed", cells.len(), index.len());
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
        let received = exchange(world, &bytes)?;
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
        Ok(())
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

    fn global_bounds(&self, world: &SimpleCommunicator, z_scale: f64) -> Result<Bounds> {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for d in &self.data {
            let p = [d[0], d[1], d[2] * z_scale];
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

        let mut counts = HashMap::with_capacity(wanted.len());
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
    let send: Vec<T> = per_rank.iter().flatten().cloned().collect();
    let total: usize = recv_counts.iter().map(|&c| c as usize).sum();
    let mut recv = vec![T::default(); total];
    {
        let out = Partition::new(&send[..], &send_counts[..], &send_displs[..]);
        let mut into = PartitionMut::new(&mut recv[..], &recv_counts[..], &recv_displs[..]);
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
    let mut all = vec![T::default(); total];
    {
        let mut into = PartitionMut::new(&mut all[..], &counts[..], &displs[..]);
        world.all_gather_varcount_into(mine, &mut into);
    }
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
}

/// Argument count of each `PS*` intrinsic. The compiler checks against it.
pub const PS_INTRINSICS: &[(&str, usize)] = &[
    ("PSLOAD", 9),
    ("PSBUILD", 2),
    ("PSGET", 10),
    ("PSLISTS", 8),
    ("PSPUT", 6),
    ("PSSCALED", 3),
    ("PSSIZES", 3),
    ("PSLEAFLOC", 1),
    ("PSUP", 3),
    ("PSDOWN", 3),
    ("PSMSYNC", 2),
    ("PSLSYNC", 2),
    ("PSHALO", 0),
    ("PSBOUNDS", 4),
    ("PSPREV", 2),
    ("PSPOS", 2),
    ("PSSUM", 3),
    ("PSALL", 9),
];

impl crate::mpi::RosyMPIContext {
    /// Run one `PS*` intrinsic. Every rank must make the same call.
    ///
    /// - `PSLOAD x y z px py pz f n count`: give the store the first
    ///   `count` entries of this rank's arrays. Particle numbers continue
    ///   from the last load, lower ranks first.
    /// - `PSBUILD q zscale`: recompute keys with z multiplied by `zscale`,
    ///   move particles to their owners, rebuild leaves, fetch the halo.
    /// - `PSGET x y z px py pz f n ids nown`: fill the arrays with this
    ///   rank's particles, then its halo copies. `ids` are particle
    ///   numbers from 1. `nown` is how many entries are owned.
    /// - `PSLISTS snumbs start_snumbs nnumbs start_nnumbs cx cy cz nleaves`:
    ///   per-leaf source and neighbour lists as positions (from 1) into the
    ///   `PSGET` arrays, with the offsets of each leaf's run and the leaf
    ///   centres in the unit cube. Sources are in particle-number order.
    ///   Neighbours come box by box, each box in particle-number order.
    /// - `PSPUT x y z px py pz`: store the first `nown` entries back.
    /// - `PSSCALED sx sy sz`: positions in the unit cube the tree was built
    ///   in, for the same entries as `PSGET`.
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
    /// - `PSBOUNDS xmin ymin zmin dzero`: the cube of the last `PSBUILD`.
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
    /// - `PSALL x y z px py pz f n count`: every particle in number order
    ///   on the first rank, nothing elsewhere. For output only.
    pub fn ps_call(&mut self, name: &str, args: &mut [&mut dyn PsCell]) -> Result<()> {
        let expected = PS_INTRINSICS.iter().find(|(n, _)| *n == name).map(|(_, c)| *c);
        ensure!(expected == Some(args.len()), "{name}: wrong number of arguments");
        let store = &mut self.pstore;
        let world = &self.world;
        match name {
            "PSLOAD" => {
                let count = args[8].ps_scalar()? as usize;
                let columns: Vec<Vec<f64>> =
                    args[..8].iter().map(|a| a.ps_values()).collect::<Result<_>>()?;
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
                let z_scale = args[1].ps_scalar()?;
                // The first build has no work estimates yet, so it balances
                // by count. Build once to get them, then again to use them.
                let passes = if store.leaves.is_empty() { 2 } else { 1 };
                for _ in 0..passes {
                    store.rebuild(world, q, z_scale)?;
                    store.build_plan(world)?;
                    store.exchange_halo(world)?;
                }
                store.fetch_previous(world)?;
            }
            "PSGET" => {
                let all = || store.data.iter().chain(&store.halo_data).chain(&store.far_data);
                for (field, arg) in args[..8].iter_mut().enumerate() {
                    arg.ps_set_values(all().map(|d| d[field]).collect())?;
                }
                let ids = store.id.iter().chain(&store.halo_id).chain(&store.far_id);
                args[8].ps_set_values(ids.map(|&id| (id + 1) as f64).collect())?;
                args[9].ps_set_scalar(store.len() as f64)?;
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
                for (field, arg) in args.iter().enumerate() {
                    let values = arg.ps_values()?;
                    ensure!(values.len() >= store.len(), "PSPUT: array is shorter than the owned particles");
                    for (d, v) in store.data.iter_mut().zip(values) {
                        d[field] = v;
                    }
                }
            }
            "PSSCALED" => {
                let bounds = store.bounds.ok_or_else(|| anyhow::anyhow!("PSSCALED before PSBUILD"))?;
                let scale = store.z_scale;
                let all = || store.data.iter().chain(&store.halo_data).chain(&store.far_data);
                for (axis, arg) in args.iter_mut().enumerate() {
                    arg.ps_set_values(
                        all().map(|d| bounds.scale([d[0], d[1], d[2] * scale])[axis]).collect(),
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
            }
            "PSMSYNC" => {
                let level = args[1].ps_scalar()? as u32;
                let plan = &store.plan;
                store.sync_cells(world, args[0].ps_cells()?, level, &plan.multipole_users, &plan.ghost_multipoles, &plan.multipole_index)?;
            }
            "PSLSYNC" => {
                let level = args[1].ps_scalar()? as u32;
                let plan = &store.plan;
                store.sync_cells(world, args[0].ps_cells()?, level, &plan.local_users, &plan.ghost_locals, &plan.local_index)?;
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
                for (field, arg) in args[..8].iter_mut().enumerate() {
                    arg.ps_set_values(order.iter().map(|&i| values[FIELDS * i + field]).collect())?;
                }
                args[8].ps_set_scalar(ids.len() as f64)?;
            }
            other => bail!("unknown particle-store intrinsic {other}"),
        }
        Ok(())
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
