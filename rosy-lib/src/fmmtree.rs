//! Linear octree for PHAD's fast multipole method.
//!
//! This builds the same tree as the group's `fmmcpp` program, from the same
//! Morton keys and the same refinement rule, but without pointers or files.
//! Particles are kept sorted by key, so every box is a contiguous range and
//! a box is identified by `(level, number)` alone. That is what lets a rank
//! own one key range and find the owner of any box by a search over the
//! range boundaries. See `docs/distributed-storage.md`.
//!
//! Agreement with `fmmcpp` is exact: same leaf set, same box centres, same
//! particle order inside every source list.

use anyhow::{Result, bail, ensure};

/// Bits per axis in a key. `fmmcpp` calls this `PRECISION`.
pub const KEY_LEVELS: u32 = 20;
/// Deepest level a leaf may sit at. `fmmcpp` keeps box numbers down to
/// level 19 only, and stops when a box there would have to divide.
pub const FINEST_LEVEL: u32 = KEY_LEVELS - 1;
/// Refinement starts here: every level-2 box is examined.
const START_LEVEL: u32 = 2;

/// The cube that holds every particle, as `fmmcpp`'s `findBoxBoundary`
/// computes it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: [f64; 3],
    /// Side length: the largest extent plus `1e-11`, so scaled
    /// coordinates stay below 1.
    pub dzero: f64,
}

impl Bounds {
    pub fn of(points: &[[f64; 3]]) -> Result<Self> {
        ensure!(!points.is_empty(), "cannot bound an empty particle set");
        let mut min = points[0];
        let mut max = points[0];
        for p in &points[1..] {
            for a in 0..3 {
                if p[a] > max[a] {
                    max[a] = p[a];
                }
                if p[a] < min[a] {
                    min[a] = p[a];
                }
            }
        }
        Ok(Self::from_extent(min, max))
    }

    /// From a global minimum and maximum, however they were found.
    pub fn from_extent(min: [f64; 3], max: [f64; 3]) -> Self {
        let (x, y, z) = (max[0] - min[0], max[1] - min[1], max[2] - min[2]);
        let mut dzero = if x > y { x } else { y };
        if z > dzero {
            dzero = z;
        }
        dzero += 0.00000000001;
        Bounds { min, dzero }
    }

    /// Position in the unit cube.
    pub fn scale(&self, p: [f64; 3]) -> [f64; 3] {
        [
            (p[0] - self.min[0]) / self.dzero,
            (p[1] - self.min[1]) / self.dzero,
            (p[2] - self.min[2]) / self.dzero,
        ]
    }
}

/// 60-bit Morton key of a point in the unit cube: x, then y, then z, from
/// the top bit down. `box_number(key, l)` is `fmmcpp`'s `giveBoxNumber`.
pub fn morton_key(scaled: [f64; 3]) -> u64 {
    let factor = (1u64 << KEY_LEVELS) as f64;
    let cell = |v: f64| (v * factor) as i64 as u64;
    interleave([cell(scaled[0]), cell(scaled[1]), cell(scaled[2])], KEY_LEVELS)
}

/// Number of the level-`level` box that holds `key`.
#[inline]
pub fn box_number(key: u64, level: u32) -> u64 {
    key >> ((KEY_LEVELS - level) * 3)
}

/// Keys inside box `(level, number)`: `first..end`.
#[inline]
pub fn key_range(level: u32, number: u64) -> (u64, u64) {
    let shift = (KEY_LEVELS - level) * 3;
    (number << shift, (number + 1) << shift)
}

/// Box number of the cell at `level` (the inverse of [`box_cell`]).
pub fn interleave_cell(cell: [u64; 3], level: u32) -> u64 {
    interleave(cell, level)
}

fn interleave(cell: [u64; 3], bits: u32) -> u64 {
    let mut out = 0u64;
    for i in (0..bits).rev() {
        out = (out << 1) | ((cell[0] >> i) & 1);
        out = (out << 1) | ((cell[1] >> i) & 1);
        out = (out << 1) | ((cell[2] >> i) & 1);
    }
    out
}

/// Integer grid position of a box at its level.
pub fn box_cell(level: u32, number: u64) -> [u64; 3] {
    let mut cell = [0u64; 3];
    for i in (0..level).rev() {
        let triple = number >> (3 * i);
        cell[0] = (cell[0] << 1) | ((triple >> 2) & 1);
        cell[1] = (cell[1] << 1) | ((triple >> 1) & 1);
        cell[2] = (cell[2] << 1) | (triple & 1);
    }
    cell
}

/// Centre of a box in the unit cube. Exact: only powers of two are involved.
pub fn box_centre(level: u32, number: u64) -> [f64; 3] {
    let side = 1.0 / (1u64 << level) as f64;
    let cell = box_cell(level, number);
    [
        (cell[0] as f64 + 0.5) * side,
        (cell[1] as f64 + 0.5) * side,
        (cell[2] as f64 + 0.5) * side,
    ]
}

/// The box and its same-level neighbours that lie inside the cube, in
/// `fmmcpp`'s order: x outermost, then y, then z, each from low to high.
/// The box itself is in the list.
pub fn neighbours(level: u32, number: u64) -> Vec<u64> {
    let cell = box_cell(level, number);
    let last = (1u64 << level) - 1;
    let span = |c: u64| -> (u64, u64) {
        (if c == 0 { 0 } else { c - 1 }, if c == last { last } else { c + 1 })
    };
    let (x, y, z) = (span(cell[0]), span(cell[1]), span(cell[2]));
    let mut out = Vec::with_capacity(27);
    for i in x.0..=x.1 {
        for j in y.0..=y.1 {
            for k in z.0..=z.1 {
                out.push(interleave([i, j, k], level));
            }
        }
    }
    out
}

/// A leaf box. Its particles are slots `first..first + count`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leaf {
    pub level: u32,
    pub number: u64,
    pub first: usize,
    pub count: usize,
}

/// Particles sorted by key, and the leaves that tile them.
#[derive(Debug, Clone)]
pub struct LeafTable {
    /// Keys in ascending order, one per slot.
    pub keys: Vec<u64>,
    /// Particle index held in each slot. Equal keys are in index order.
    pub order: Vec<usize>,
    /// Leaves in key order.
    pub leaves: Vec<Leaf>,
}

impl LeafTable {
    /// Build the tree for `keys[i]`, the key of particle `i`.
    ///
    /// The rule is `fmmcpp`'s `tagAndDivide` with every particle both a
    /// source and a target. A box is examined when its parent was examined
    /// and had to divide. An examined box that holds particles is a leaf
    /// when the same-level boxes around it, itself included, hold at most
    /// `q` particles. Otherwise its children are examined.
    pub fn build(keys: &[u64], q: usize) -> Result<Self> {
        let mut order: Vec<usize> = (0..keys.len()).collect();
        order.sort_unstable_by_key(|&i| (keys[i], i));
        let sorted: Vec<u64> = order.iter().map(|&i| keys[i]).collect();
        let mut table = LeafTable {
            keys: sorted,
            order,
            leaves: Vec::new(),
        };

        let mut examined = table.occupied_boxes(START_LEVEL, 0, table.keys.len());
        let mut level = START_LEVEL;
        while !examined.is_empty() {
            let mut next = Vec::new();
            for &number in &examined {
                let around: usize = neighbours(level, number)
                    .into_iter()
                    .map(|n| {
                        let (a, b) = table.slots(level, n);
                        b - a
                    })
                    .sum();
                let (first, end) = table.slots(level, number);
                if around <= q {
                    table.leaves.push(Leaf {
                        level,
                        number,
                        first,
                        count: end - first,
                    });
                } else {
                    if level >= FINEST_LEVEL {
                        bail!(
                            "FMM tree reached level {level} with more than q={q} particles \
                             around one box; q is too small for this distribution"
                        );
                    }
                    next.extend(table.occupied_boxes(level + 1, first, end));
                }
            }
            examined = next;
            level += 1;
        }
        table.leaves.sort_unstable_by_key(|leaf| leaf.first);
        Ok(table)
    }

    /// Slots `first..end` whose keys fall in box `(level, number)`.
    pub fn slots(&self, level: u32, number: u64) -> (usize, usize) {
        let (lo, hi) = key_range(level, number);
        (
            self.keys.partition_point(|&k| k < lo),
            self.keys.partition_point(|&k| k < hi),
        )
    }

    /// Distinct level-`level` boxes among slots `first..end`, ascending.
    fn occupied_boxes(&self, level: u32, first: usize, end: usize) -> Vec<u64> {
        let mut out = Vec::new();
        let mut at = first;
        while at < end {
            let number = box_number(self.keys[at], level);
            out.push(number);
            let (_, hi) = key_range(level, number);
            at += self.keys[at..end].partition_point(|&k| k < hi);
        }
        out
    }

    /// Particles in box `(level, number)` in index order, which is the
    /// order `fmmcpp` lists them and so the order sums over them run.
    pub fn sources(&self, level: u32, number: u64) -> Vec<usize> {
        let (first, end) = self.slots(level, number);
        let mut out = self.order[first..end].to_vec();
        out.sort_unstable();
        out
    }

    /// Particles in the boxes around a leaf, the leaf itself left out.
    /// Boxes come in `neighbours` order, each in index order.
    pub fn neighbour_sources(&self, leaf: &Leaf) -> Vec<usize> {
        let mut out = Vec::new();
        for number in neighbours(leaf.level, leaf.number) {
            if number != leaf.number {
                out.extend(self.sources(leaf.level, number));
            }
        }
        out
    }

    /// Leaves in the order `fmmcpp` numbers them: deepest level first,
    /// ascending box number inside a level.
    pub fn leaves_deepest_first(&self) -> Vec<Leaf> {
        let mut out = self.leaves.clone();
        out.sort_unstable_by_key(|leaf| (std::cmp::Reverse(leaf.level), leaf.number));
        out
    }
}

/// Which boxes take part in the upward and downward passes.
///
/// `fmmcpp` writes this as its D-tree, C_L set and C-forest. Here it is
/// three sorted lists per level, and everything else is derived per box.
#[derive(Debug, Clone, Default)]
pub struct FmmPlan {
    /// Boxes that get a local expansion: the leaves and their ancestors
    /// down from level 2. Indexed by level.
    pub targets: Vec<Vec<u64>>,
    /// The targets that had to divide, which are the ancestors.
    pub divided: Vec<Vec<u64>>,
    /// Boxes whose multipole some target needs. Indexed by level.
    pub multipoles: Vec<Vec<u64>>,
}

/// How one multipole expansion is put together, in summation order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultipoleRecipe {
    /// The box was never divided: sum its particles about its centre.
    Particles(Vec<usize>),
    /// The box was divided. First shift the multipoles of `children`,
    /// then add the particles of each `direct` child about this box's
    /// centre. A `direct` child holds particles but no target needs its
    /// own multipole.
    Children {
        children: Vec<u64>,
        direct: Vec<(u64, Vec<usize>)>,
    },
}

impl FmmPlan {
    /// True when the octree split box `(level, number)` into children.
    /// `fmmcpp` splits a box when it or a same-level neighbour had to
    /// divide, and always splits levels 0 and 1.
    pub fn is_split(&self, level: u32, number: u64) -> bool {
        if level < START_LEVEL {
            return true;
        }
        let Some(divided) = self.divided.get(level as usize) else {
            return false;
        };
        neighbours(level, number)
            .iter()
            .any(|n| divided.binary_search(n).is_ok())
    }

    pub fn needs_multipole(&self, level: u32, number: u64) -> bool {
        self.multipoles
            .get(level as usize)
            .is_some_and(|boxes| boxes.binary_search(&number).is_ok())
    }
}

impl LeafTable {
    /// Particles in box `(level, number)`.
    pub fn count(&self, level: u32, number: u64) -> usize {
        let (first, end) = self.slots(level, number);
        end - first
    }

    /// Boxes whose multipoles feed the local expansion of target
    /// `(level, number)`: children of the parent's neighbours that hold
    /// particles and do not touch the target. The order is `fmmcpp`'s.
    pub fn interaction_list(&self, level: u32, number: u64) -> Vec<u64> {
        let touching = neighbours(level, number);
        let mut out = Vec::new();
        for near_parent in neighbours(level - 1, number >> 3) {
            for child in near_parent * 8..near_parent * 8 + 8 {
                if !touching.contains(&child) && self.count(level, child) > 0 {
                    out.push(child);
                }
            }
        }
        out
    }

    pub fn plan(&self) -> FmmPlan {
        use std::collections::BTreeSet;
        let depth = self.leaves.iter().map(|l| l.level).max().unwrap_or(0) as usize + 1;
        let mut targets = vec![BTreeSet::new(); depth];
        let mut divided = vec![BTreeSet::new(); depth];
        for leaf in &self.leaves {
            targets[leaf.level as usize].insert(leaf.number);
            let mut number = leaf.number;
            for level in (START_LEVEL..leaf.level).rev() {
                number >>= 3;
                if !divided[level as usize].insert(number) {
                    break;
                }
                targets[level as usize].insert(number);
            }
        }
        let mut multipoles = vec![BTreeSet::new(); depth];
        for (level, boxes) in targets.iter().enumerate() {
            for &number in boxes {
                multipoles[level].extend(self.interaction_list(level as u32, number));
            }
        }
        let flat = |sets: Vec<BTreeSet<u64>>| sets.into_iter().map(|s| s.into_iter().collect()).collect();
        FmmPlan {
            targets: flat(targets),
            divided: flat(divided),
            multipoles: flat(multipoles),
        }
    }

    pub fn multipole_recipe(&self, plan: &FmmPlan, level: u32, number: u64) -> MultipoleRecipe {
        if !plan.is_split(level, number) {
            return MultipoleRecipe::Particles(self.sources(level, number));
        }
        let mut children = Vec::new();
        let mut direct = Vec::new();
        for child in number * 8..number * 8 + 8 {
            if self.count(level + 1, child) == 0 {
                continue;
            }
            if plan.needs_multipole(level + 1, child) {
                children.push(child);
            } else {
                direct.push((child, self.sources(level + 1, child)));
            }
        }
        MultipoleRecipe::Children { children, direct }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_x_then_y_then_z() {
        // Top bit of x only, then y only, then z only.
        assert_eq!(box_number(morton_key([0.5, 0.0, 0.0]), 1), 4);
        assert_eq!(box_number(morton_key([0.0, 0.5, 0.0]), 1), 2);
        assert_eq!(box_number(morton_key([0.0, 0.0, 0.5]), 1), 1);
        assert_eq!(box_cell(2, box_number(morton_key([0.8, 0.3, 0.6]), 2)), [3, 1, 2]);
    }

    #[test]
    fn neighbours_clip_at_the_cube() {
        assert_eq!(neighbours(2, 0).len(), 8);
        let inner = interleave([1, 1, 1], 2);
        let around = neighbours(2, inner);
        assert_eq!(around.len(), 27);
        assert_eq!(around[13], inner);
        assert_eq!(around[0], interleave([0, 0, 0], 2));
        assert_eq!(around[1], interleave([0, 0, 1], 2));
    }

    #[test]
    fn leaves_tile_every_particle_once() {
        // A deterministic cloud, denser near one corner.
        let mut state = 12345u64;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((state >> 11) as f64) / (1u64 << 53) as f64
        };
        let points: Vec<[f64; 3]> = (0..4000)
            .map(|_| {
                let (a, b, c) = (next(), next(), next());
                [a * a, b * b, c]
            })
            .collect();
        let bounds = Bounds::of(&points).unwrap();
        let keys: Vec<u64> = points.iter().map(|p| morton_key(bounds.scale(*p))).collect();
        let table = LeafTable::build(&keys, 60).unwrap();

        let mut at = 0;
        for leaf in &table.leaves {
            assert_eq!(leaf.first, at, "leaves must be contiguous");
            assert!(leaf.count > 0);
            assert_eq!(table.slots(leaf.level, leaf.number), (at, at + leaf.count));
            let around = leaf.count + table.neighbour_sources(leaf).len();
            assert!(around <= 60, "leaf neighbourhood holds {around}");
            at += leaf.count;
        }
        assert_eq!(at, points.len());
    }

    /// The passes' structure against `fmmcpp`'s `file_3.bin` and
    /// `Tree_file_*.bin`, dumped as `planN.txt`. Needs `ROSY_FMMTREE_REF`.
    #[test]
    fn plan_matches_fmmcpp_reference() {
        let Ok(dir) = std::env::var("ROSY_FMMTREE_REF") else {
            return;
        };
        let nums = |s: &str| -> Vec<u64> { s.split_whitespace().map(|t| t.parse().unwrap()).collect() };
        let ids = |s: &str, sep: char| -> Vec<usize> {
            s.split(sep).filter(|t| !t.is_empty()).map(|t| t.parse::<usize>().unwrap() - 1).collect()
        };
        let mut checked = 0;
        for n in [5000usize, 50000] {
            let Ok(text) = std::fs::read_to_string(format!("{dir}/pos{n}.dat")) else {
                continue;
            };
            let points: Vec<[f64; 3]> = text
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| {
                    let v: Vec<f64> = l.split_whitespace().map(|t| t.parse().unwrap()).collect();
                    [v[0], v[1], v[2]]
                })
                .collect();
            let bounds = Bounds::of(&points).unwrap();
            let keys: Vec<u64> = points.iter().map(|p| morton_key(bounds.scale(*p))).collect();
            let table = LeafTable::build(&keys, 60).unwrap();
            let plan = table.plan();

            let reference = std::fs::read_to_string(format!("{dir}/plan{n}.txt")).unwrap();
            let (mut targets, mut multipoles) = (0, 0);
            for line in reference.lines() {
                let parts: Vec<&str> = line.split('|').collect();
                let head: Vec<&str> = parts[0].split_whitespace().collect();
                let level: u32 = head[1].parse().unwrap();
                let number: u64 = head[2].parse().unwrap();
                if head[0] == "D" {
                    assert!(plan.targets[level as usize].binary_search(&number).is_ok(), "target {level}/{number}, n={n}");
                    if level > 2 {
                        assert_eq!(head[3].parse::<u64>().unwrap(), number >> 3);
                    }
                    assert_eq!(table.interaction_list(level, number), nums(parts[1]), "interactions of {level}/{number}, n={n}");
                    targets += 1;
                } else {
                    assert!(plan.needs_multipole(level, number), "multipole {level}/{number}, n={n}");
                    let want = if parts[1] == "L" {
                        MultipoleRecipe::Particles(ids(parts[2], ' '))
                    } else {
                        MultipoleRecipe::Children {
                            children: nums(parts[2]),
                            direct: parts[3]
                                .split_whitespace()
                                .map(|t| {
                                    let (child, list) = t.split_once(':').unwrap();
                                    (child.parse().unwrap(), ids(list, ','))
                                })
                                .collect(),
                        }
                    };
                    assert_eq!(table.multipole_recipe(&plan, level, number), want, "recipe of {level}/{number}, n={n}");
                    multipoles += 1;
                }
            }
            assert_eq!(targets, plan.targets.iter().map(|l| l.len()).sum::<usize>(), "target count, n={n}");
            assert_eq!(multipoles, plan.multipoles.iter().map(|l| l.len()).sum::<usize>(), "multipole count, n={n}");
            checked += 1;
        }
        assert!(checked > 0, "no reference files found in {dir}");
    }

    /// Box-for-box comparison with `fmmcpp`. Set `ROSY_FMMTREE_REF` to a
    /// directory holding `posN.dat` (x y z per line) and `refN.txt` (the
    /// dump of `fmmcpp`'s `file_1.bin` and `file_4.bin`) to run it.
    #[test]
    fn matches_fmmcpp_reference() {
        let Ok(dir) = std::env::var("ROSY_FMMTREE_REF") else {
            return;
        };
        let hex = |s: &str| -> f64 {
            // C99 hex float: [-]0x1.hhhhp[+-]d
            let (neg, s) = match s.strip_prefix('-') {
                Some(rest) => (true, rest),
                None => (false, s),
            };
            let s = s.strip_prefix("0x").expect("hex float");
            let (mant, exp) = s.split_once('p').expect("exponent");
            let (int, frac) = mant.split_once('.').unwrap_or((mant, ""));
            let mut value = u64::from_str_radix(int, 16).unwrap() as f64;
            let mut scale = 1.0 / 16.0;
            for c in frac.chars() {
                value += c.to_digit(16).unwrap() as f64 * scale;
                scale /= 16.0;
            }
            let value = value * 2f64.powi(exp.parse().unwrap());
            if neg { -value } else { value }
        };
        let mut checked = 0;
        for n in [5000usize, 50000] {
            let Ok(text) = std::fs::read_to_string(format!("{dir}/pos{n}.dat")) else {
                continue;
            };
            let points: Vec<[f64; 3]> = text
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| {
                    let v: Vec<f64> = l.split_whitespace().map(|t| t.parse().unwrap()).collect();
                    [v[0], v[1], v[2]]
                })
                .collect();
            let reference = std::fs::read_to_string(format!("{dir}/ref{n}.txt")).unwrap();
            let mut lines = reference.lines();
            let head: Vec<&str> = lines.next().unwrap().split_whitespace().collect();

            let bounds = Bounds::of(&points).unwrap();
            assert_eq!(bounds.min, [hex(head[0]), hex(head[1]), hex(head[2])]);
            assert_eq!(bounds.dzero, hex(head[3]));
            let keys: Vec<u64> = points.iter().map(|p| morton_key(bounds.scale(*p))).collect();
            let table = LeafTable::build(&keys, 60).unwrap();
            let leaves = table.leaves_deepest_first();
            assert_eq!(leaves.len(), head[4].parse::<usize>().unwrap(), "leaf count, n={n}");

            let ids = |s: &str| -> Vec<usize> {
                s.split_whitespace().map(|t| t.parse::<usize>().unwrap() - 1).collect()
            };
            for (i, (leaf, line)) in leaves.iter().zip(lines).enumerate() {
                let parts: Vec<&str> = line.split('|').collect();
                let centre: Vec<f64> = parts[0].split_whitespace().map(hex).collect();
                assert_eq!(box_centre(leaf.level, leaf.number).to_vec(), centre, "centre of leaf {i}, n={n}");
                assert_eq!(table.sources(leaf.level, leaf.number), ids(parts[1]), "sources of leaf {i}, n={n}");
                assert_eq!(table.neighbour_sources(leaf), ids(parts[2]), "neighbours of leaf {i}, n={n}");
            }
            checked += 1;
        }
        assert!(checked > 0, "no reference files found in {dir}");
    }
}
