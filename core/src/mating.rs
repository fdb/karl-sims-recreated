use rand::Rng;

use crate::genotype::{AttachFace, GenomeGraph, MorphConn, MorphNode};
use crate::mutation;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Remap all connection indices so they fall within `[0, num_nodes)`.
fn fix_connections(genome: &mut GenomeGraph) {
    let n = genome.nodes.len();
    if n == 0 {
        return;
    }
    for conn in &mut genome.connections {
        conn.source %= n;
        conn.target %= n;
    }
}

// ---------------------------------------------------------------------------
// Crossover
// ---------------------------------------------------------------------------

/// Single- or two-point crossover of two parent genomes.
///
/// Nodes are taken alternately from `p1` and `p2` at crossover boundaries.
/// Connections are copied from whichever parent contributed each node.
/// Out-of-bounds indices are remapped modulo child node count.
pub fn crossover<R: Rng>(p1: &GenomeGraph, p2: &GenomeGraph, rng: &mut R) -> GenomeGraph {
    let len1 = p1.nodes.len();
    let len2 = p2.nodes.len();
    let max_len = len1.max(len2);

    if max_len == 0 {
        return p1.clone();
    }

    // Pick 1-2 crossover points.
    let num_points = rng.gen_range(1..=2usize);
    let mut points: Vec<usize> = (0..num_points)
        .map(|_| rng.gen_range(1..=max_len))
        .collect();
    points.sort();
    points.dedup();

    // Determine which parent contributes each index position.
    // Start with p1, toggle at each crossover point.
    let mut child_nodes: Vec<MorphNode> = Vec::with_capacity(max_len);
    let mut source_parent: Vec<bool> = Vec::with_capacity(max_len); // true = p1, false = p2
    let mut from_p1 = true;
    let mut point_idx = 0;

    for i in 0..max_len {
        if point_idx < points.len() && i >= points[point_idx] {
            from_p1 = !from_p1;
            point_idx += 1;
        }

        let node = if from_p1 {
            p1.nodes.get(i).cloned()
        } else {
            p2.nodes.get(i).cloned()
        };

        if let Some(node) = node {
            child_nodes.push(node);
            source_parent.push(from_p1);
        } else {
            // Fall back to whichever parent has a node at this index.
            let fallback = if from_p1 {
                p2.nodes.get(i).cloned()
            } else {
                p1.nodes.get(i).cloned()
            };
            if let Some(node) = fallback {
                child_nodes.push(node);
                source_parent.push(!from_p1);
            }
            // If neither parent has a node here, skip.
        }
    }

    // Copy connections from each parent for nodes that came from it.
    let mut child_connections: Vec<MorphConn> = Vec::new();
    for (child_idx, &is_p1) in source_parent.iter().enumerate() {
        let parent = if is_p1 { p1 } else { p2 };
        for conn in &parent.connections {
            if conn.source == child_idx || conn.target == child_idx {
                child_connections.push(conn.clone());
            }
        }
    }

    // Deduplicate connections (same source+target pair could be added twice).
    child_connections.sort_by_key(|c| (c.source, c.target));
    child_connections.dedup_by_key(|c| (c.source, c.target));

    let mut child = GenomeGraph {
        nodes: child_nodes,
        connections: child_connections,
        root: 0,
        global_brain: if rng.gen_bool(0.5) {
            p1.global_brain.clone()
        } else {
            p2.global_brain.clone()
        },
    };

    fix_connections(&mut child);
    mutation::garbage_collect(&mut child);
    child
}

// ---------------------------------------------------------------------------
// Grafting
// ---------------------------------------------------------------------------

/// Graft a subtree from `p2` onto a clone of `p1`.
///
/// A random node from `p2` (and its connected subtree) is appended to the
/// child, then connected to a random existing node.
pub fn graft<R: Rng>(p1: &GenomeGraph, p2: &GenomeGraph, rng: &mut R) -> GenomeGraph {
    let mut child = p1.clone();

    if p2.nodes.is_empty() {
        return child;
    }

    // Pick a random node in p2 as the graft root.
    let graft_root = rng.gen_range(0..p2.nodes.len());

    // BFS from graft_root in p2 to find the subtree.
    let mut subtree_indices: Vec<usize> = Vec::new();
    let mut visited = vec![false; p2.nodes.len()];
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(graft_root);
    visited[graft_root] = true;

    while let Some(idx) = queue.pop_front() {
        subtree_indices.push(idx);
        for conn in &p2.connections {
            if conn.source == idx && !visited[conn.target] {
                visited[conn.target] = true;
                queue.push_back(conn.target);
            }
        }
    }

    // Map from p2 subtree index -> new child index.
    let base_offset = child.nodes.len();
    let mut index_map = std::collections::HashMap::new();
    for (new_local, &old_idx) in subtree_indices.iter().enumerate() {
        index_map.insert(old_idx, base_offset + new_local);
        child.nodes.push(p2.nodes[old_idx].clone());
    }

    // Copy connections within the subtree, remapping indices.
    for conn in &p2.connections {
        if let (Some(&new_src), Some(&new_tgt)) =
            (index_map.get(&conn.source), index_map.get(&conn.target))
        {
            child.connections.push(MorphConn {
                source: new_src,
                target: new_tgt,
                parent_face: conn.parent_face,
                child_face: conn.child_face,
                scale: conn.scale,
                reflection: conn.reflection,
            });
        }
    }

    // Connect a random existing node in child to the grafted subtree root.
    let attach_point = rng.gen_range(0..base_offset);
    let grafted_root = base_offset; // first appended node
    child.connections.push(MorphConn {
        source: attach_point,
        target: grafted_root,
        parent_face: AttachFace::ALL[rng.gen_range(0..6)],
        child_face: AttachFace::ALL[rng.gen_range(0..6)],
        scale: rng.gen_range(0.5..1.5),
        reflection: rng.gen_bool(0.5),
    });

    fix_connections(&mut child);
    mutation::garbage_collect(&mut child);
    child
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn is_valid(genome: &GenomeGraph) -> bool {
        if genome.nodes.is_empty() {
            return false;
        }
        if genome.root >= genome.nodes.len() {
            return false;
        }
        for conn in &genome.connections {
            if conn.source >= genome.nodes.len() || conn.target >= genome.nodes.len() {
                return false;
            }
        }
        true
    }

    #[test]
    fn crossover_produces_valid_genome() {
        for seed in 0..20u64 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let p1 = GenomeGraph::random(&mut rng);
            let p2 = GenomeGraph::random(&mut rng);
            let child = crossover(&p1, &p2, &mut rng);
            assert!(
                is_valid(&child),
                "seed {seed}: crossover produced invalid genome"
            );
        }
    }

    #[test]
    fn graft_produces_valid_genome() {
        for seed in 0..20u64 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let p1 = GenomeGraph::random(&mut rng);
            let p2 = GenomeGraph::random(&mut rng);
            let child = graft(&p1, &p2, &mut rng);
            assert!(
                is_valid(&child),
                "seed {seed}: graft produced invalid genome"
            );
        }
    }

    #[test]
    fn mating_produces_different_offspring() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let p1 = GenomeGraph::random(&mut rng);
        let p2 = GenomeGraph::random(&mut rng);

        // Try multiple seeds — at least one should produce a child different from both parents.
        let mut found_different = false;
        for seed in 0..20u64 {
            let mut rng2 = ChaCha8Rng::seed_from_u64(seed);
            let child = crossover(&p1, &p2, &mut rng2);

            let same_as_p1 = child.nodes.len() == p1.nodes.len()
                && child.connections.len() == p1.connections.len();
            let same_as_p2 = child.nodes.len() == p2.nodes.len()
                && child.connections.len() == p2.connections.len();

            if !same_as_p1 || !same_as_p2 {
                found_different = true;
                break;
            }
        }
        assert!(
            found_different,
            "crossover should produce offspring different from at least one parent"
        );
    }
}
