//! Checks the distributed particle store against the single-process tree.
//!
//! Run under MPI with any rank count:
//! `mpirun -np 8 cargo run -p rosy-lib --features mpi --release --example pstore_check -- 20000`
//!
//! Every rank count must give the same leaves, holding the same particles,
//! as `LeafTable::build` on one process.

use mpi::traits::*;
use rosy_lib::fmmtree::{Bounds, LeafTable, morton_key};
use rosy_lib::fmmtree::MultipoleRecipe;
use rosy_lib::pstore::{BoxRecipe, FIELDS, Frame, ParticleStore, gather_all};
use std::collections::HashMap;

const Q: usize = 60;
/// A beam moving mostly along z, a little across, as PHAD's common velocity
/// (`W = gamma*beta`, `relg = gamma = sqrt(1 + W.W)`).
const W: [f64; 3] = [0.031, -0.017, 1.8392];

fn frame() -> Frame {
    let relg = (1.0 + W[0] * W[0] + W[1] * W[1] + W[2] * W[2]).sqrt();
    Frame { w: W, relg }
}

/// Position of particle `id` at `step`. A clustered cloud that drifts.
fn point(id: u64, step: u64) -> [f64; 3] {
    let mut s = id.wrapping_mul(0x9E3779B97F4A7C15) ^ 0xD1B54A32D192ED03;
    let mut next = || {
        s ^= s >> 30;
        s = s.wrapping_mul(0xBF58476D1CE4E5B9);
        s ^= s >> 27;
        s = s.wrapping_mul(0x94D049BB133111EB);
        s ^= s >> 31;
        (s >> 11) as f64 / (1u64 << 53) as f64
    };
    let (a, b, c, d) = (next(), next(), next(), next());
    let t = step as f64 * 0.013;
    [a * a + t * (d - 0.5), b * b * b - t * c, c + t * a]
}

/// The tag carried in field `e` (index 8): any value tied to the particle.
fn tag(id: u64, step: u64) -> f64 {
    (id * 7 + step) as f64 + 0.25
}

fn check(world: &mpi::topology::SimpleCommunicator, store: &ParticleStore, n: u64, step: u64) {
    // `e` travels with its particle through migration and halo copies.
    for (i, d) in store.data.iter().enumerate() {
        assert_eq!(d[8], tag(store.id[i], step), "e of owned particle {}", store.id[i]);
    }
    for (i, d) in store.halo_data.iter().enumerate() {
        assert_eq!(d[8], tag(store.halo_id[i], step), "e of halo copy {}", store.halo_id[i]);
    }
    let mut triples = Vec::new();
    let mut ids = Vec::new();
    let mut around = Vec::new();
    let mut at = 0;
    for leaf in &store.leaves {
        assert_eq!(leaf.first, at, "local leaves must tile the local slots");
        triples.extend([leaf.level as u64, leaf.number, leaf.count as u64]);
        let mut members = store.id[leaf.first..leaf.first + leaf.count].to_vec();
        members.sort_unstable();
        ids.extend(members);
        let near = store.neighbour_positions(leaf);
        around.push(near.len() as u64);
        around.extend(near.iter().map(|&p| store.id_at(p)));
        at += leaf.count;
    }
    assert_eq!(at, store.len());
    let triples = gather_all(world, &triples).unwrap();
    let ids = gather_all(world, &ids).unwrap();
    let around = gather_all(world, &around).unwrap();
    let sizes = gather_all(world, &[store.len() as u64]).unwrap();
    let halos = gather_all(world, &[store.halo_id.len() as u64]).unwrap();

    // The passes: every target with its interaction list, and every
    // multipole with what it is built from.
    let ids_of = |level: u32, number: u64| -> Vec<u64> {
        store.box_positions(level, number).iter().map(|&p| store.id_at(p)).collect()
    };
    let mut lists = Vec::new();
    for (level, boxes) in store.plan.targets.iter().enumerate() {
        for &number in boxes {
            let list = store.interaction_list(level as u32, number);
            lists.extend([level as u64, number, list.len() as u64]);
            lists.extend(list);
        }
    }
    let mut recipes = Vec::new();
    for (level, boxes) in store.plan.multipoles.iter().enumerate() {
        for &number in boxes {
            recipes.extend([level as u64, number]);
            match store.multipole_recipe(level as u32, number) {
                BoxRecipe::Particles => {
                    let ids = ids_of(level as u32, number);
                    recipes.extend([0, ids.len() as u64]);
                    recipes.extend(ids);
                }
                BoxRecipe::Children { children, direct } => {
                    recipes.extend([1, children.len() as u64]);
                    recipes.extend(children);
                    recipes.push(direct.len() as u64);
                    for child in direct {
                        let ids = ids_of(level as u32 + 1, child);
                        recipes.extend([child, ids.len() as u64]);
                        recipes.extend(ids);
                    }
                }
            }
        }
    }
    let lists = gather_all(world, &lists).unwrap();
    let recipes = gather_all(world, &recipes).unwrap();
    let ghosts = gather_all(world, &[store.plan.ghost_multipoles.len() as u64]).unwrap();
    if world.rank() != 0 {
        return;
    }
    let points: Vec<[f64; 3]> = (0..n)
        .map(|id| {
            let p = point(id, step);
            frame().boost(p[0], p[1], p[2])
        })
        .collect();
    let bounds = Bounds::of(&points).unwrap();
    assert_eq!(Some(bounds), store.bounds, "bounding cube");
    let keys: Vec<u64> = points.iter().map(|p| morton_key(bounds.scale(*p))).collect();
    let serial = LeafTable::build(&keys, Q).unwrap();
    assert_eq!(triples.len() / 3, serial.leaves.len(), "leaf count");
    let mut at = 0;
    let mut near_at = 0;
    for (i, leaf) in serial.leaves.iter().enumerate() {
        let got = &triples[3 * i..3 * i + 3];
        assert_eq!(got, [leaf.level as u64, leaf.number, leaf.count as u64], "leaf {i}");
        let want: Vec<u64> = serial.sources(leaf.level, leaf.number).iter().map(|&p| p as u64).collect();
        assert_eq!(&ids[at..at + leaf.count], &want[..], "particles of leaf {i}");
        at += leaf.count;
        let want: Vec<u64> = serial.neighbour_sources(leaf).iter().map(|&p| p as u64).collect();
        let len = around[near_at] as usize;
        assert_eq!(&around[near_at + 1..near_at + 1 + len], &want[..], "neighbours of leaf {i}");
        near_at += 1 + len;
    }
    let plan = serial.plan();
    let mut got_lists = HashMap::new();
    let mut i = 0;
    while i < lists.len() {
        let len = lists[i + 2] as usize;
        let old = got_lists.insert((lists[i] as u32, lists[i + 1]), lists[i + 3..i + 3 + len].to_vec());
        assert!(old.is_none(), "target {}/{} is on two ranks", lists[i], lists[i + 1]);
        i += 3 + len;
    }
    let mut targets = 0;
    for (level, boxes) in plan.targets.iter().enumerate() {
        for &number in boxes {
            let want = serial.interaction_list(level as u32, number);
            assert_eq!(got_lists.get(&(level as u32, number)), Some(&want), "interactions of {level}/{number}");
            targets += 1;
        }
    }
    assert_eq!(targets, got_lists.len(), "target count");

    let mut got_recipes = HashMap::new();
    let mut i = 0;
    while i < recipes.len() {
        let key = (recipes[i] as u32, recipes[i + 1]);
        let take = |at: &mut usize| -> Vec<usize> {
            let len = recipes[*at] as usize;
            let out = recipes[*at + 1..*at + 1 + len].iter().map(|&v| v as usize).collect();
            *at += 1 + len;
            out
        };
        let mut at = i + 3;
        let recipe = if recipes[i + 2] == 0 {
            MultipoleRecipe::Particles(take(&mut at))
        } else {
            let children = take(&mut at).into_iter().map(|c| c as u64).collect();
            let n_direct = recipes[at] as usize;
            at += 1;
            let mut direct = Vec::new();
            for _ in 0..n_direct {
                let child = recipes[at];
                at += 1;
                direct.push((child, take(&mut at)));
            }
            MultipoleRecipe::Children { children, direct }
        };
        assert!(got_recipes.insert(key, recipe).is_none(), "multipole {key:?} is on two ranks");
        i = at;
    }
    let mut multipoles = 0;
    for (level, boxes) in plan.multipoles.iter().enumerate() {
        for &number in boxes {
            let want = serial.multipole_recipe(&plan, level as u32, number);
            assert_eq!(got_recipes.get(&(level as u32, number)), Some(&want), "recipe of {level}/{number}");
            multipoles += 1;
        }
    }
    assert_eq!(multipoles, got_recipes.len(), "multipole count");
    println!(
        "        {targets} targets and {multipoles} multipoles match; multipole copies {} ({:.0}%)",
        ghosts.iter().sum::<u64>(),
        100.0 * ghosts.iter().sum::<u64>() as f64 / multipoles as f64
    );
    let (lo, hi) = (sizes.iter().min().unwrap(), sizes.iter().max().unwrap());
    println!(
        "step {step}: {} ranks, {} leaves and their neighbour lists match; particles per rank {lo}..{hi}, halo copies {} ({:.0}% of n)",
        sizes.len(),
        serial.leaves.len(),
        halos.iter().sum::<u64>(),
        100.0 * halos.iter().sum::<u64>() as f64 / n as f64
    );
}

fn main() {
    let universe = mpi::initialize().unwrap();
    let world = universe.world();
    let n: u64 = std::env::args().nth(1).map(|s| s.parse().unwrap()).unwrap_or(20000);
    let (rank, ranks) = (world.rank() as u64, world.size() as u64);

    let mut store = ParticleStore::default();
    for id in (0..n).filter(|id| id % ranks == rank) {
        let p = point(id, 0);
        let mut data = [0.0; FIELDS];
        data[..3].copy_from_slice(&p);
        data[8] = tag(id, 0);
        store.push(id, data);
    }
    for step in 0..4 {
        for i in 0..store.len() {
            let p = point(store.id[i], step);
            store.data[i][..3].copy_from_slice(&p);
            store.data[i][8] = tag(store.id[i], step);
        }
        store.rebuild(&world, Q, frame()).unwrap();
        store.build_plan(&world).unwrap();
        store.exchange_halo(&world).unwrap();
        check(&world, &store, n, step);
    }
}
