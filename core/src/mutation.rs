use std::collections::VecDeque;

use glam::DVec3;
use rand::Rng;

use crate::genotype::{
    AttachFace, BrainGraph, BrainNode, GenomeGraph, MorphConn, MorphNode,
    NeuronFunc, NeuronInput,
};
use crate::joint::JointType;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Gaussian-ish perturbation: `value += rng.gen_range(-1.0..1.0) * scale * value.abs().max(0.1)`.
fn perturb<R: Rng>(rng: &mut R, value: f64, scale: f64) -> f64 {
    value + rng.gen_range(-1.0..1.0) * scale * value.abs().max(0.1)
}

fn random_joint_type<R: Rng>(rng: &mut R) -> JointType {
    match rng.gen_range(0..7) {
        0 => JointType::Rigid,
        1 => JointType::Revolute,
        2 => JointType::Twist,
        3 => JointType::Universal,
        4 => JointType::BendTwist,
        5 => JointType::TwistBend,
        _ => JointType::Spherical,
    }
}

fn random_neuron_func<R: Rng>(rng: &mut R) -> NeuronFunc {
    match rng.gen_range(0..6) {
        0 => NeuronFunc::Sum,
        1 => NeuronFunc::Product,
        2 => NeuronFunc::Sigmoid,
        3 => NeuronFunc::Sin,
        4 => NeuronFunc::OscillateWave,
        _ => NeuronFunc::Memory,
    }
}

fn random_face<R: Rng>(rng: &mut R) -> AttachFace {
    AttachFace::ALL[rng.gen_range(0..6)]
}

// ---------------------------------------------------------------------------
// 1. Node parameter mutation
// ---------------------------------------------------------------------------

fn mutate_node_params<R: Rng>(genome: &mut GenomeGraph, rng: &mut R, scale: f64) {
    for node in &mut genome.nodes {
        // Dimensions: Gaussian perturbation, clamp [0.05, 2.0]
        let dx = perturb(rng, node.dimensions.x, scale);
        let dy = perturb(rng, node.dimensions.y, scale);
        let dz = perturb(rng, node.dimensions.z, scale);
        node.dimensions = DVec3::new(
            dx.clamp(0.05, 2.0),
            dy.clamp(0.05, 2.0),
            dz.clamp(0.05, 2.0),
        );

        // Joint type: small probability
        if rng.r#gen::<f64>() < 0.1 * scale {
            node.joint_type = random_joint_type(rng);
        }

        // Joint limits: Gaussian perturbation
        for i in 0..3 {
            node.joint_limit_min[i] = perturb(rng, node.joint_limit_min[i], scale).clamp(-3.14, 0.0);
            node.joint_limit_max[i] = perturb(rng, node.joint_limit_max[i], scale).clamp(0.0, 3.14);
        }

        // Recursive limit: occasionally ±1, clamp [1, 5]
        if rng.r#gen::<f64>() < 0.1 * scale {
            let delta: i32 = if rng.gen_bool(0.5) { 1 } else { -1 };
            node.recursive_limit = (node.recursive_limit as i32 + delta).clamp(1, 5) as u32;
        }

        // Terminal-only: flip with small probability
        if rng.r#gen::<f64>() < 0.1 * scale {
            node.terminal_only = !node.terminal_only;
        }
    }
}

// ---------------------------------------------------------------------------
// 2. New random node
// ---------------------------------------------------------------------------

fn maybe_add_node<R: Rng>(genome: &mut GenomeGraph, rng: &mut R, scale: f64) {
    if rng.r#gen::<f64>() < scale {
        let joint_type = random_joint_type(rng);
        let brain = BrainGraph::random_for_joint(rng, joint_type);
        genome.nodes.push(MorphNode {
            dimensions: DVec3::new(
                rng.gen_range(0.1..0.8),
                rng.gen_range(0.1..0.8),
                rng.gen_range(0.1..0.8),
            ),
            joint_type,
            joint_limit_min: [-1.0; 3],
            joint_limit_max: [1.0; 3],
            recursive_limit: rng.gen_range(1..=4),
            terminal_only: rng.gen_bool(0.3),
            brain,
        });
        // Node starts disconnected; connection mutations may link it later.
    }
}

// ---------------------------------------------------------------------------
// 3. Connection parameter mutation
// ---------------------------------------------------------------------------

fn mutate_connection_params<R: Rng>(genome: &mut GenomeGraph, rng: &mut R, scale: f64) {
    let n = genome.nodes.len();
    for conn in &mut genome.connections {
        // Scale: Gaussian perturbation, clamp [0.3, 3.0]
        conn.scale = perturb(rng, conn.scale, scale).clamp(0.3, 3.0);

        // Reflection: flip with small probability
        if rng.r#gen::<f64>() < 0.1 * scale {
            conn.reflection = !conn.reflection;
        }

        // Move target to a different random node with small probability
        if rng.r#gen::<f64>() < 0.1 * scale && n > 1 {
            let new_target = rng.gen_range(0..n);
            conn.target = new_target;
        }
    }
}

// ---------------------------------------------------------------------------
// 4. Random connection add/remove
// ---------------------------------------------------------------------------

fn mutate_connections<R: Rng>(genome: &mut GenomeGraph, rng: &mut R, scale: f64) {
    let n = genome.nodes.len();

    // Add: for each node, with probability `scale`, add connection to random other node
    if n > 1 {
        for source in 0..n {
            if rng.r#gen::<f64>() < scale {
                let target = rng.gen_range(0..n);
                genome.connections.push(MorphConn {
                    source,
                    target,
                    parent_face: random_face(rng),
                    child_face: random_face(rng),
                    scale: rng.gen_range(0.5..1.5),
                    reflection: rng.gen_bool(0.5),
                });
            }
        }
    }

    // Remove: for each existing connection, with probability `scale`, remove it
    // but don't remove if it would disconnect the graph.
    let mut i = 0;
    while i < genome.connections.len() {
        if rng.r#gen::<f64>() < scale {
            // Check if removing this connection would disconnect the graph
            let removed = genome.connections.swap_remove(i);
            if !is_connected(genome) {
                // Put it back
                genome.connections.push(removed);
                // swap_remove moved the last element to position i, and we just pushed
                // the removed one to the end. We need to fix: swap them back.
                let last = genome.connections.len() - 1;
                if i < last {
                    genome.connections.swap(i, last);
                }
                i += 1;
            }
            // If removal was fine, don't increment i (swap_remove moved next element here)
        } else {
            i += 1;
        }
    }
}

/// Check if all nodes reachable from root via connections (treating as undirected).
fn is_connected(genome: &GenomeGraph) -> bool {
    if genome.nodes.is_empty() {
        return true;
    }
    let n = genome.nodes.len();
    let mut visited = vec![false; n];
    let mut queue = VecDeque::new();
    queue.push_back(genome.root);
    visited[genome.root] = true;

    while let Some(node) = queue.pop_front() {
        for conn in &genome.connections {
            if conn.source == node && !visited[conn.target] {
                visited[conn.target] = true;
                queue.push_back(conn.target);
            }
            if conn.target == node && !visited[conn.source] {
                visited[conn.source] = true;
                queue.push_back(conn.source);
            }
        }
    }
    visited.iter().all(|&v| v)
}

// ---------------------------------------------------------------------------
// 5. Garbage collection
// ---------------------------------------------------------------------------

pub fn garbage_collect(genome: &mut GenomeGraph) {
    if genome.nodes.is_empty() {
        return;
    }

    // BFS from root following connections (directed: source -> target).
    let n = genome.nodes.len();
    let mut reachable = vec![false; n];
    let mut queue = VecDeque::new();
    queue.push_back(genome.root);
    reachable[genome.root] = true;

    while let Some(node) = queue.pop_front() {
        for conn in &genome.connections {
            if conn.source == node && !reachable[conn.target] {
                reachable[conn.target] = true;
                queue.push_back(conn.target);
            }
        }
    }

    // Build index mapping: old index -> new index (or None if removed).
    let mut index_map: Vec<Option<usize>> = vec![None; n];
    let mut new_idx = 0;
    for old_idx in 0..n {
        if reachable[old_idx] {
            index_map[old_idx] = Some(new_idx);
            new_idx += 1;
        }
    }

    // Remove unreachable nodes (iterate in reverse to preserve indices).
    let mut new_nodes = Vec::with_capacity(new_idx);
    for (old_idx, node) in genome.nodes.drain(..).enumerate() {
        if reachable[old_idx] {
            new_nodes.push(node);
        }
    }
    genome.nodes = new_nodes;

    // Update root.
    genome.root = index_map[genome.root].expect("root must be reachable");

    // Remove connections that reference removed nodes, and remap indices.
    genome.connections.retain(|conn| {
        index_map[conn.source].is_some() && index_map[conn.target].is_some()
    });
    for conn in &mut genome.connections {
        conn.source = index_map[conn.source].unwrap();
        conn.target = index_map[conn.target].unwrap();
    }
}

// ---------------------------------------------------------------------------
// Brain mutation
// ---------------------------------------------------------------------------

fn mutate_brain<R: Rng>(brain: &mut BrainGraph, rng: &mut R, scale: f64) {
    let n_neurons = brain.neurons.len();

    // Mutate each neuron.
    for neuron in &mut brain.neurons {
        // Perturb input weights.
        for (_input, weight) in &mut neuron.inputs {
            *weight = perturb(rng, *weight, scale).clamp(-10.0, 10.0);
        }

        // With small probability, change function type.
        if rng.r#gen::<f64>() < 0.1 * scale {
            neuron.func = random_neuron_func(rng);
        }

        // With small probability, add an input.
        if rng.r#gen::<f64>() < 0.1 * scale && neuron.inputs.len() < 4 {
            let input = random_neuron_input(rng, n_neurons);
            let weight = rng.gen_range(-1.0..1.0);
            neuron.inputs.push((input, weight));
        }

        // With small probability, remove an input.
        if rng.r#gen::<f64>() < 0.1 * scale && neuron.inputs.len() > 1 {
            let idx = rng.gen_range(0..neuron.inputs.len());
            neuron.inputs.swap_remove(idx);
        }
    }

    // Mutate each effector.
    for effector in &mut brain.effectors {
        effector.weight = perturb(rng, effector.weight, scale).clamp(-10.0, 10.0);
    }

    // With small probability, add a new neuron.
    if rng.r#gen::<f64>() < 0.1 * scale {
        let new_n = brain.neurons.len();
        let input = random_neuron_input(rng, new_n);
        let weight = rng.gen_range(-1.0..1.0);
        brain.neurons.push(BrainNode {
            func: random_neuron_func(rng),
            inputs: vec![(input, weight)],
        });
    }
}

fn random_neuron_input<R: Rng>(rng: &mut R, n_neurons: usize) -> NeuronInput {
    if n_neurons > 0 && rng.gen_bool(0.5) {
        NeuronInput::Neuron(rng.gen_range(0..n_neurons))
    } else if rng.gen_bool(0.5) {
        NeuronInput::Sensor(rng.gen_range(0..4))
    } else {
        NeuronInput::Constant(rng.gen_range(-2.0..2.0))
    }
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Mutate a genome in place using the 5 operators from Sims' paper.
pub fn mutate<R: Rng>(genome: &mut GenomeGraph, rng: &mut R) {
    let graph_size = genome.nodes.len().max(1);
    let mutation_scale = 1.0 / graph_size as f64;

    mutate_node_params(genome, rng, mutation_scale);
    maybe_add_node(genome, rng, mutation_scale);
    mutate_connection_params(genome, rng, mutation_scale);
    mutate_connections(genome, rng, mutation_scale);
    garbage_collect(genome);

    for node in &mut genome.nodes {
        mutate_brain(&mut node.brain, rng, mutation_scale);
    }
    mutate_brain(&mut genome.global_brain, rng, mutation_scale);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn mutation_changes_genome() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let original = GenomeGraph::random(&mut rng);
        let mut mutated = original.clone();

        // Apply several rounds to ensure at least some difference.
        for _ in 0..5 {
            mutate(&mut mutated, &mut rng);
        }

        // Check that something changed — dimensions are always perturbed so
        // it's virtually impossible they're identical after 5 rounds.
        let mut differs = false;
        if original.nodes.len() != mutated.nodes.len()
            || original.connections.len() != mutated.connections.len()
        {
            differs = true;
        } else {
            for (a, b) in original.nodes.iter().zip(mutated.nodes.iter()) {
                if a.dimensions != b.dimensions
                    || a.joint_type != b.joint_type
                    || a.recursive_limit != b.recursive_limit
                    || a.terminal_only != b.terminal_only
                {
                    differs = true;
                    break;
                }
            }
        }
        assert!(differs, "mutation should change the genome");
    }

    #[test]
    fn garbage_collection_removes_unreachable() {
        let mut rng = ChaCha8Rng::seed_from_u64(99);
        let mut genome = GenomeGraph::random(&mut rng);

        let original_count = genome.nodes.len();

        // Add a disconnected node (no connections pointing to it).
        genome.nodes.push(MorphNode {
            dimensions: DVec3::new(0.5, 0.5, 0.5),
            joint_type: JointType::Rigid,
            joint_limit_min: [-1.0; 3],
            joint_limit_max: [1.0; 3],
            recursive_limit: 1,
            terminal_only: false,
            brain: BrainGraph {
                neurons: Vec::new(),
                effectors: Vec::new(),
            },
        });

        assert_eq!(genome.nodes.len(), original_count + 1);

        garbage_collect(&mut genome);

        assert_eq!(
            genome.nodes.len(),
            original_count,
            "GC should remove the unreachable node"
        );

        // Verify all connections still reference valid indices.
        for conn in &genome.connections {
            assert!(conn.source < genome.nodes.len());
            assert!(conn.target < genome.nodes.len());
        }
    }

    #[test]
    fn mutation_preserves_valid_structure() {
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let mut genome = GenomeGraph::random(&mut rng);

        for _ in 0..50 {
            mutate(&mut genome, &mut rng);

            // Root must be valid.
            assert!(
                genome.root < genome.nodes.len(),
                "root index out of bounds"
            );

            // All connections must reference valid nodes.
            for conn in &genome.connections {
                assert!(
                    conn.source < genome.nodes.len(),
                    "connection source {} out of bounds ({})",
                    conn.source,
                    genome.nodes.len()
                );
                assert!(
                    conn.target < genome.nodes.len(),
                    "connection target {} out of bounds ({})",
                    conn.target,
                    genome.nodes.len()
                );
            }

            // Brain neuron references must be valid.
            for node in &genome.nodes {
                let n = node.brain.neurons.len();
                for neuron in &node.brain.neurons {
                    for (input, _) in &neuron.inputs {
                        if let NeuronInput::Neuron(idx) = input {
                            assert!(*idx < n, "neuron input index out of bounds");
                        }
                    }
                }
                for effector in &node.brain.effectors {
                    if let NeuronInput::Neuron(idx) = &effector.input {
                        assert!(*idx < n, "effector neuron index out of bounds");
                    }
                }
            }

            // Must have at least one node.
            assert!(!genome.nodes.is_empty(), "genome must have at least one node");
        }
    }
}
