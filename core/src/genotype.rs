use glam::DVec3;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::joint::JointType;

/// Index into the nodes vector of a GenomeGraph.
pub type NodeIndex = usize;

// ---------------------------------------------------------------------------
// AttachFace
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttachFace {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

impl AttachFace {
    /// Outward-facing unit normal for this face.
    pub fn normal(&self) -> DVec3 {
        match self {
            AttachFace::PosX => DVec3::X,
            AttachFace::NegX => DVec3::NEG_X,
            AttachFace::PosY => DVec3::Y,
            AttachFace::NegY => DVec3::NEG_Y,
            AttachFace::PosZ => DVec3::Z,
            AttachFace::NegZ => DVec3::NEG_Z,
        }
    }

    /// Center point of this face given box half-extents.
    pub fn center(&self, half_extents: DVec3) -> DVec3 {
        self.normal() * half_extents
    }

    const ALL: [AttachFace; 6] = [
        AttachFace::PosX,
        AttachFace::NegX,
        AttachFace::PosY,
        AttachFace::NegY,
        AttachFace::PosZ,
        AttachFace::NegZ,
    ];

    fn random<R: Rng>(rng: &mut R) -> Self {
        AttachFace::ALL[rng.gen_range(0..6)]
    }
}

// ---------------------------------------------------------------------------
// Brain types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NeuronFunc {
    Sum,
    Product,
    Sigmoid,
    Sin,
    OscillateWave,
    Memory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NeuronInput {
    Neuron(usize),
    Sensor(usize),
    Constant(f64),
}

impl PartialEq for NeuronInput {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (NeuronInput::Neuron(a), NeuronInput::Neuron(b)) => a == b,
            (NeuronInput::Sensor(a), NeuronInput::Sensor(b)) => a == b,
            (NeuronInput::Constant(a), NeuronInput::Constant(b)) => a.to_bits() == b.to_bits(),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainNode {
    pub func: NeuronFunc,
    /// Up to 3 (input, weight) pairs.
    pub inputs: Vec<(NeuronInput, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectorNode {
    pub input: NeuronInput,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainGraph {
    pub neurons: Vec<BrainNode>,
    pub effectors: Vec<EffectorNode>,
}

impl BrainGraph {
    /// Create a minimal brain for a given joint type.
    ///
    /// For each DOF: one OscillateWave neuron (random frequency and phase)
    /// and one effector that reads from it.
    pub fn random_for_joint<R: Rng>(rng: &mut R, joint_type: JointType) -> Self {
        let dofs = joint_type.dof_count();
        let mut neurons = Vec::with_capacity(dofs);
        let mut effectors = Vec::with_capacity(dofs);

        for i in 0..dofs {
            let freq: f64 = rng.gen_range(1.0..5.0);
            let phase: f64 = rng.gen_range(0.0..std::f64::consts::TAU);

            neurons.push(BrainNode {
                func: NeuronFunc::OscillateWave,
                inputs: vec![
                    (NeuronInput::Constant(freq), 1.0),
                    (NeuronInput::Constant(phase), 1.0),
                ],
            });

            let weight: f64 = rng.gen_range(1.0..5.0);
            effectors.push(EffectorNode {
                input: NeuronInput::Neuron(i),
                weight,
            });
        }

        BrainGraph { neurons, effectors }
    }
}

// ---------------------------------------------------------------------------
// Morphology types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorphNode {
    pub dimensions: DVec3,
    pub joint_type: JointType,
    pub joint_limit_min: [f64; 3],
    pub joint_limit_max: [f64; 3],
    pub recursive_limit: u32,
    pub terminal_only: bool,
    pub brain: BrainGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorphConn {
    pub source: NodeIndex,
    pub target: NodeIndex,
    pub parent_face: AttachFace,
    pub child_face: AttachFace,
    pub scale: f64,
    pub reflection: bool,
}

// ---------------------------------------------------------------------------
// GenomeGraph
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenomeGraph {
    pub nodes: Vec<MorphNode>,
    pub connections: Vec<MorphConn>,
    pub root: NodeIndex,
    pub global_brain: BrainGraph,
}

impl GenomeGraph {
    /// Generate a random genome with 1-5 nodes.
    pub fn random<R: Rng>(rng: &mut R) -> Self {
        let node_count = rng.gen_range(1..=5usize);

        let joint_types_pool = [
            JointType::Revolute,
            JointType::Twist,
            JointType::Universal,
            JointType::BendTwist,
            JointType::Spherical,
        ];

        let mut nodes = Vec::with_capacity(node_count);
        let mut connections = Vec::new();

        for i in 0..node_count {
            let joint_type = if i == 0 {
                JointType::Rigid
            } else {
                joint_types_pool[rng.gen_range(0..joint_types_pool.len())]
            };

            let dimensions = DVec3::new(
                rng.gen_range(0.1..0.8),
                rng.gen_range(0.1..0.8),
                rng.gen_range(0.1..0.8),
            );

            let brain = BrainGraph::random_for_joint(rng, joint_type);

            nodes.push(MorphNode {
                dimensions,
                joint_type,
                joint_limit_min: [-1.0; 3],
                joint_limit_max: [1.0; 3],
                recursive_limit: rng.gen_range(1..=4),
                terminal_only: rng.gen_bool(0.3),
                brain,
            });

            if i > 0 {
                let source = rng.gen_range(0..i);
                connections.push(MorphConn {
                    source,
                    target: i,
                    parent_face: AttachFace::random(rng),
                    child_face: AttachFace::random(rng),
                    scale: rng.gen_range(0.5..1.5),
                    reflection: rng.gen_bool(0.5),
                });
            }
        }

        let global_brain = BrainGraph {
            neurons: Vec::new(),
            effectors: Vec::new(),
        };

        GenomeGraph {
            nodes,
            connections,
            root: 0,
            global_brain,
        }
    }
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
    fn random_genome_deterministic() {
        let mut rng1 = ChaCha8Rng::seed_from_u64(42);
        let mut rng2 = ChaCha8Rng::seed_from_u64(42);

        let g1 = GenomeGraph::random(&mut rng1);
        let g2 = GenomeGraph::random(&mut rng2);

        assert_eq!(g1.nodes.len(), g2.nodes.len());
        assert_eq!(g1.connections.len(), g2.connections.len());
    }

    #[test]
    fn genome_serialization_roundtrip() {
        let mut rng = ChaCha8Rng::seed_from_u64(123);
        let genome = GenomeGraph::random(&mut rng);

        let bytes = bincode::serialize(&genome).expect("serialize");
        let decoded: GenomeGraph = bincode::deserialize(&bytes).expect("deserialize");

        assert_eq!(genome.nodes.len(), decoded.nodes.len());
        assert_eq!(genome.connections.len(), decoded.connections.len());
        assert_eq!(genome.root, decoded.root);

        for (a, b) in genome.nodes.iter().zip(decoded.nodes.iter()) {
            assert_eq!(a.joint_type, b.joint_type);
            assert_eq!(a.brain.neurons.len(), b.brain.neurons.len());
            assert_eq!(a.brain.effectors.len(), b.brain.effectors.len());
        }

        for (a, b) in genome.connections.iter().zip(decoded.connections.iter()) {
            assert_eq!(a.source, b.source);
            assert_eq!(a.target, b.target);
            assert_eq!(a.parent_face, b.parent_face);
            assert_eq!(a.child_face, b.child_face);
        }
    }

    #[test]
    fn random_genome_has_valid_structure() {
        for seed in 0..20u64 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let genome = GenomeGraph::random(&mut rng);

            // Must have at least one node (the root).
            assert!(!genome.nodes.is_empty());

            // Root index must be valid.
            assert!(genome.root < genome.nodes.len());

            // Every connection must reference valid node indices.
            for conn in &genome.connections {
                assert!(conn.source < genome.nodes.len(), "seed {seed}: bad source");
                assert!(conn.target < genome.nodes.len(), "seed {seed}: bad target");
            }

            // Brain neuron inputs that reference neurons must be in range.
            for node in &genome.nodes {
                let n_neurons = node.brain.neurons.len();
                for effector in &node.brain.effectors {
                    if let NeuronInput::Neuron(idx) = &effector.input {
                        assert!(*idx < n_neurons, "seed {seed}: bad effector neuron ref");
                    }
                }
            }
        }
    }
}
