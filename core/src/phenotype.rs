use std::collections::VecDeque;

use glam::DVec3;

use crate::genotype::GenomeGraph;
use crate::joint::{Joint, JointType};
use crate::world::World;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum SensorType {
    JointAngle { joint_idx: usize, dof: usize },
}

#[derive(Debug, Clone)]
pub struct SensorInfo {
    pub body_idx: usize,
    pub sensor_type: SensorType,
}

pub struct Phenotype {
    pub world: World,
    /// (genotype node index, recursion depth) for each body in the world.
    pub body_node_map: Vec<(usize, u32)>,
    pub num_effectors: usize,
    pub sensor_map: Vec<SensorInfo>,
}

// ---------------------------------------------------------------------------
// Helper: perpendicular axes to a given normal
// ---------------------------------------------------------------------------

fn perpendicular_axes(normal: DVec3) -> (DVec3, DVec3) {
    let reference = if normal.dot(DVec3::Y).abs() < 0.9 {
        DVec3::Y
    } else {
        DVec3::X
    };
    let a = normal.cross(reference).normalize();
    let b = normal.cross(a).normalize();
    (a, b)
}

// ---------------------------------------------------------------------------
// develop()
// ---------------------------------------------------------------------------

/// Grow a genotype directed graph into a physics World.
pub fn develop(genome: &GenomeGraph) -> Phenotype {
    let mut world = World::new();
    world.water_enabled = true;
    world.gravity = DVec3::ZERO;

    let mut body_node_map: Vec<(usize, u32)> = Vec::new();
    let mut sensor_map: Vec<SensorInfo> = Vec::new();

    // Track how many times each genotype node has been instantiated (for recursive_limit).
    let mut visit_count: Vec<u32> = vec![0; genome.nodes.len()];

    // Root body
    let root_node = &genome.nodes[genome.root];
    let root_body = world.add_body(root_node.dimensions * 0.5);
    world.root = root_body;
    body_node_map.push((genome.root, 0));
    visit_count[genome.root] += 1;

    // BFS queue: (genotype node index, body index in world, recursion depth)
    let mut queue: VecDeque<(usize, usize, u32)> = VecDeque::new();
    queue.push_back((genome.root, root_body, 0));

    while let Some((geno_node_idx, parent_body_idx, depth)) = queue.pop_front() {
        // Find all connections from this genotype node.
        for conn in &genome.connections {
            if conn.source != geno_node_idx {
                continue;
            }

            let target_node = &genome.nodes[conn.target];

            // Check recursive limit.
            if visit_count[conn.target] >= target_node.recursive_limit {
                continue;
            }

            // Check terminal_only: if set, only instantiate when parent has no further children
            // beyond this. In practice we interpret terminal_only as: skip if depth == 0
            // (i.e., the target only appears at the tips). We use a simpler interpretation:
            // terminal_only nodes are only placed if parent is not the root (depth > 0).
            if target_node.terminal_only && depth == 0 {
                continue;
            }

            visit_count[conn.target] += 1;
            let child_depth = depth + 1;

            // Compute child half-extents: node dimensions scaled by connection scale, halved.
            let child_half_extents = target_node.dimensions * conn.scale * 0.5;
            let parent_half_extents = world.bodies[parent_body_idx].half_extents;

            let child_body = world.add_body(child_half_extents);
            body_node_map.push((conn.target, child_depth));

            // Compute anchors.
            let parent_anchor = conn.parent_face.center(parent_half_extents);
            let child_anchor = conn.child_face.center(child_half_extents);

            // Compute joint axes perpendicular to parent face normal.
            let normal = conn.parent_face.normal();
            let (axis_a, axis_b) = perpendicular_axes(normal);

            // Create joint based on target node's joint type.
            let joint = match target_node.joint_type {
                JointType::Rigid => Joint::rigid(parent_body_idx, child_body, parent_anchor, child_anchor),
                JointType::Revolute => Joint::revolute(parent_body_idx, child_body, parent_anchor, child_anchor, axis_a),
                JointType::Twist => Joint::twist(parent_body_idx, child_body, parent_anchor, child_anchor, normal),
                JointType::Universal => Joint::universal(parent_body_idx, child_body, parent_anchor, child_anchor, axis_a, axis_b),
                JointType::BendTwist => Joint::bend_twist(parent_body_idx, child_body, parent_anchor, child_anchor, axis_a, normal),
                JointType::TwistBend => Joint::twist_bend(parent_body_idx, child_body, parent_anchor, child_anchor, normal, axis_a),
                JointType::Spherical => Joint::spherical(parent_body_idx, child_body, parent_anchor, child_anchor),
            };

            // Set joint limits from genotype.
            let joint_idx = world.add_joint(joint);
            for dof in 0..target_node.joint_type.dof_count() {
                world.joints[joint_idx].angle_min[dof] = target_node.joint_limit_min[dof];
                world.joints[joint_idx].angle_max[dof] = target_node.joint_limit_max[dof];
            }

            // Record sensors: one JointAngle sensor per DOF.
            for dof in 0..target_node.joint_type.dof_count() {
                sensor_map.push(SensorInfo {
                    body_idx: child_body,
                    sensor_type: SensorType::JointAngle {
                        joint_idx,
                        dof,
                    },
                });
            }

            // Continue traversal from child.
            queue.push_back((conn.target, child_body, child_depth));
        }
    }

    // Count total effectors across all body parts.
    let num_effectors: usize = body_node_map
        .iter()
        .map(|(node_idx, _)| genome.nodes[*node_idx].brain.effectors.len())
        .sum();

    world.forward_kinematics();

    Phenotype {
        world,
        body_node_map,
        num_effectors,
        sensor_map,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genotype::*;
    use crate::joint::JointType;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn develop_single_node_genome() {
        let genome = GenomeGraph {
            nodes: vec![MorphNode {
                dimensions: DVec3::new(0.5, 0.3, 0.4),
                joint_type: JointType::Rigid,
                joint_limit_min: [-1.0; 3],
                joint_limit_max: [1.0; 3],
                recursive_limit: 1,
                terminal_only: false,
                brain: BrainGraph {
                    neurons: Vec::new(),
                    effectors: Vec::new(),
                },
            }],
            connections: Vec::new(),
            root: 0,
            global_brain: BrainGraph {
                neurons: Vec::new(),
                effectors: Vec::new(),
            },
        };

        let pheno = develop(&genome);
        assert_eq!(pheno.world.bodies.len(), 1, "single node → 1 body");
        assert_eq!(pheno.world.joints.len(), 0, "single node → 0 joints");
        assert_eq!(pheno.sensor_map.len(), 0, "single node → 0 sensors");
        assert_eq!(pheno.body_node_map.len(), 1);
        assert_eq!(pheno.body_node_map[0], (0, 0));
    }

    #[test]
    fn develop_random_genome_produces_valid_world() {
        for seed in 0..20u64 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let genome = GenomeGraph::random(&mut rng);
            let pheno = develop(&genome);

            assert!(
                !pheno.world.bodies.is_empty(),
                "seed {seed}: must have at least 1 body"
            );

            // Every joint must reference valid body indices.
            for (ji, joint) in pheno.world.joints.iter().enumerate() {
                assert!(
                    joint.parent_idx < pheno.world.bodies.len(),
                    "seed {seed}: joint {ji} bad parent"
                );
                assert!(
                    joint.child_idx < pheno.world.bodies.len(),
                    "seed {seed}: joint {ji} bad child"
                );
            }

            // Sensor joint indices must be valid.
            for sensor in &pheno.sensor_map {
                let SensorType::JointAngle { joint_idx, dof } = sensor.sensor_type;
                assert!(
                    joint_idx < pheno.world.joints.len(),
                    "seed {seed}: bad sensor joint_idx"
                );
                assert!(
                    dof < pheno.world.joints[joint_idx].joint_type.dof_count(),
                    "seed {seed}: bad sensor dof"
                );
            }
        }
    }

    #[test]
    fn develop_genome_with_connection() {
        let genome = GenomeGraph {
            nodes: vec![
                MorphNode {
                    dimensions: DVec3::new(0.6, 0.4, 0.4),
                    joint_type: JointType::Rigid, // root joint type doesn't matter
                    joint_limit_min: [-1.0; 3],
                    joint_limit_max: [1.0; 3],
                    recursive_limit: 1,
                    terminal_only: false,
                    brain: BrainGraph {
                        neurons: Vec::new(),
                        effectors: Vec::new(),
                    },
                },
                MorphNode {
                    dimensions: DVec3::new(0.4, 0.3, 0.3),
                    joint_type: JointType::Revolute,
                    joint_limit_min: [-0.5, 0.0, 0.0],
                    joint_limit_max: [0.5, 0.0, 0.0],
                    recursive_limit: 1,
                    terminal_only: false,
                    brain: BrainGraph {
                        neurons: vec![BrainNode {
                            func: NeuronFunc::Sum,
                            inputs: vec![(NeuronInput::Constant(1.0), 1.0)],
                        }],
                        effectors: vec![EffectorNode {
                            input: NeuronInput::Neuron(0),
                            weight: 1.0,
                        }],
                    },
                },
            ],
            connections: vec![MorphConn {
                source: 0,
                target: 1,
                parent_face: AttachFace::PosX,
                child_face: AttachFace::NegX,
                scale: 1.0,
                reflection: false,
            }],
            root: 0,
            global_brain: BrainGraph {
                neurons: Vec::new(),
                effectors: Vec::new(),
            },
        };

        let pheno = develop(&genome);
        assert_eq!(pheno.world.bodies.len(), 2, "2 nodes → 2 bodies");
        assert_eq!(pheno.world.joints.len(), 1, "1 connection → 1 joint");
        assert_eq!(
            pheno.world.joints[0].joint_type,
            JointType::Revolute,
            "revolute joint"
        );
        assert_eq!(pheno.sensor_map.len(), 1, "revolute → 1 sensor (1 DOF)");

        // Joint limits should match genotype.
        assert!((pheno.world.joints[0].angle_min[0] - (-0.5)).abs() < 1e-10);
        assert!((pheno.world.joints[0].angle_max[0] - 0.5).abs() < 1e-10);
    }
}
