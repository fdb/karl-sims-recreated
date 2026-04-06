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
    PhotoSensor { body_idx: usize, axis: usize },
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

/// A deferred growth step: all the info needed to add one body segment
/// to a creature mid-simulation, following the BFS expansion order.
#[derive(Debug, Clone)]
pub struct GrowthStep {
    /// Index of the genotype node for this body.
    pub geno_node_idx: usize,
    /// Index of the parent body in the world (already instantiated).
    pub parent_body_idx: usize,
    /// Connection from the genome that produced this growth step.
    pub conn_idx: usize,
    /// Recursion depth of this body.
    pub depth: u32,
}

/// A plan for incremental growth: root is developed immediately,
/// remaining bodies are queued for later instantiation.
#[derive(Debug, Clone)]
pub struct GrowthPlan {
    pub steps: Vec<GrowthStep>,
    pub next_step: usize,
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

    // Photosensors for root body (3 axes: X, Y, Z).
    for axis in 0..3 {
        sensor_map.push(SensorInfo {
            body_idx: root_body,
            sensor_type: SensorType::PhotoSensor { body_idx: root_body, axis },
        });
    }

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
            // Clamp each axis to a minimum of 0.03 m (3 cm) so bodies are large
            // enough for Rapier's contact solver to handle cleanly. Without this
            // floor, dim 0.05 × scale 0.3 × 0.5 = 0.0075 m (7.5 mm) — well
            // below the solver's comfort zone, causing jittery contacts, thin
            // plates that slide anomalously, and degenerate inertia tensors.
            const MIN_HALF_EXTENT: f64 = 0.03;
            let child_half_extents = (target_node.dimensions * conn.scale * 0.5)
                .max(glam::DVec3::splat(MIN_HALF_EXTENT));
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

            // Photosensors for child body (3 axes: X, Y, Z).
            for axis in 0..3 {
                sensor_map.push(SensorInfo {
                    body_idx: child_body,
                    sensor_type: SensorType::PhotoSensor { body_idx: child_body, axis },
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

/// Develop only the root body, returning a growth plan for the remaining
/// bodies. Used by the developmental growth system.
pub fn develop_with_growth_plan(genome: &GenomeGraph) -> (Phenotype, GrowthPlan) {
    let mut world = World::new();
    world.water_enabled = true;
    world.gravity = DVec3::ZERO;

    let mut body_node_map: Vec<(usize, u32)> = Vec::new();
    let mut sensor_map: Vec<SensorInfo> = Vec::new();

    let mut visit_count: Vec<u32> = vec![0; genome.nodes.len()];

    // Root body
    let root_node = &genome.nodes[genome.root];
    let root_body = world.add_body(root_node.dimensions * 0.5);
    world.root = root_body;
    body_node_map.push((genome.root, 0));
    visit_count[genome.root] += 1;

    for axis in 0..3 {
        sensor_map.push(SensorInfo {
            body_idx: root_body,
            sensor_type: SensorType::PhotoSensor { body_idx: root_body, axis },
        });
    }

    // BFS to build the growth plan (same order as develop(), but deferred).
    let mut growth_steps: Vec<GrowthStep> = Vec::new();
    let mut queue: VecDeque<(usize, usize, u32)> = VecDeque::new();
    queue.push_back((genome.root, root_body, 0));

    while let Some((geno_node_idx, parent_body_idx, depth)) = queue.pop_front() {
        for (ci, conn) in genome.connections.iter().enumerate() {
            if conn.source != geno_node_idx {
                continue;
            }
            let target_node = &genome.nodes[conn.target];
            if visit_count[conn.target] >= target_node.recursive_limit {
                continue;
            }
            if target_node.terminal_only && depth == 0 {
                continue;
            }
            visit_count[conn.target] += 1;
            let child_depth = depth + 1;

            // The body index for this child will be determined when it's actually
            // grown, but we need the parent's body index (which is already known).
            growth_steps.push(GrowthStep {
                geno_node_idx: conn.target,
                parent_body_idx,
                conn_idx: ci,
                depth: child_depth,
            });

            // For future children of this node, the parent_body_idx will be
            // updated when this step is actually executed. We use a placeholder
            // for now -- the index will be `body_node_map.len()` at execution time.
            // We need to continue the BFS using a "virtual" body index.
            let virtual_body_idx = root_body + growth_steps.len();
            queue.push_back((conn.target, virtual_body_idx, child_depth));
        }
    }

    let num_effectors = genome.nodes[genome.root].brain.effectors.len();
    world.forward_kinematics();

    let pheno = Phenotype {
        world,
        body_node_map,
        num_effectors,
        sensor_map,
    };

    let plan = GrowthPlan {
        steps: growth_steps,
        next_step: 0,
    };

    (pheno, plan)
}

/// Execute one growth step: add a body and joint to the world.
///
/// Updates `body_node_map`, `sensor_map`, and returns the new body index.
/// The parent body index in the growth step must be valid (already instantiated).
pub fn grow_one_step(
    genome: &GenomeGraph,
    world: &mut World,
    body_node_map: &mut Vec<(usize, u32)>,
    sensor_map: &mut Vec<SensorInfo>,
    step: &GrowthStep,
) -> usize {
    let conn = &genome.connections[step.conn_idx];
    let target_node = &genome.nodes[step.geno_node_idx];

    const MIN_HALF_EXTENT: f64 = 0.03;
    let child_half_extents = (target_node.dimensions * conn.scale * 0.5)
        .max(DVec3::splat(MIN_HALF_EXTENT));
    let parent_half_extents = world.bodies[step.parent_body_idx].half_extents;

    // Position the new body adjacent to its parent, at the joint attachment point.
    let parent_anchor = conn.parent_face.center(parent_half_extents);
    let child_anchor = conn.child_face.center(child_half_extents);
    let parent_tf = world.transforms[step.parent_body_idx];
    let joint_pos = parent_tf.transform_point3(parent_anchor);
    let child_pos = joint_pos - child_anchor; // Approximate: ignores rotation

    let child_body = world.add_body_dynamic(child_half_extents);
    world.transforms[child_body] = glam::DAffine3::from_translation(child_pos);
    body_node_map.push((step.geno_node_idx, step.depth));

    // Create joint.
    let normal = conn.parent_face.normal();
    let (axis_a, axis_b) = perpendicular_axes(normal);
    let joint = match target_node.joint_type {
        crate::joint::JointType::Rigid => Joint::rigid(step.parent_body_idx, child_body, parent_anchor, child_anchor),
        crate::joint::JointType::Revolute => Joint::revolute(step.parent_body_idx, child_body, parent_anchor, child_anchor, axis_a),
        crate::joint::JointType::Twist => Joint::twist(step.parent_body_idx, child_body, parent_anchor, child_anchor, normal),
        crate::joint::JointType::Universal => Joint::universal(step.parent_body_idx, child_body, parent_anchor, child_anchor, axis_a, axis_b),
        crate::joint::JointType::BendTwist => Joint::bend_twist(step.parent_body_idx, child_body, parent_anchor, child_anchor, axis_a, normal),
        crate::joint::JointType::TwistBend => Joint::twist_bend(step.parent_body_idx, child_body, parent_anchor, child_anchor, normal, axis_a),
        crate::joint::JointType::Spherical => Joint::spherical(step.parent_body_idx, child_body, parent_anchor, child_anchor),
    };

    let joint_idx = world.add_joint_dynamic(joint);
    for dof in 0..target_node.joint_type.dof_count() {
        world.joints[joint_idx].angle_min[dof] = target_node.joint_limit_min[dof];
        world.joints[joint_idx].angle_max[dof] = target_node.joint_limit_max[dof];
    }

    // Record sensors.
    for dof in 0..target_node.joint_type.dof_count() {
        sensor_map.push(SensorInfo {
            body_idx: child_body,
            sensor_type: SensorType::JointAngle { joint_idx, dof },
        });
    }
    for axis in 0..3 {
        sensor_map.push(SensorInfo {
            body_idx: child_body,
            sensor_type: SensorType::PhotoSensor { body_idx: child_body, axis },
        });
    }

    child_body
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
                    signal_effectors: Vec::new(),
                },
            }],
            connections: Vec::new(),
            root: 0,
            global_brain: BrainGraph {
                neurons: Vec::new(),
                effectors: Vec::new(),
                signal_effectors: Vec::new(),
            },
        };

        let pheno = develop(&genome);
        assert_eq!(pheno.world.bodies.len(), 1, "single node → 1 body");
        assert_eq!(pheno.world.joints.len(), 0, "single node → 0 joints");
        assert_eq!(pheno.sensor_map.len(), 3, "single node → 3 photosensors");
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
                match sensor.sensor_type {
                    SensorType::JointAngle { joint_idx, dof } => {
                        assert!(
                            joint_idx < pheno.world.joints.len(),
                            "seed {seed}: bad sensor joint_idx"
                        );
                        assert!(
                            dof < pheno.world.joints[joint_idx].joint_type.dof_count(),
                            "seed {seed}: bad sensor dof"
                        );
                    }
                    SensorType::PhotoSensor { body_idx, axis } => {
                        assert!(
                            body_idx < pheno.world.bodies.len(),
                            "seed {seed}: bad photosensor body_idx"
                        );
                        assert!(axis < 3, "seed {seed}: bad photosensor axis");
                    }
                }
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
                        signal_effectors: Vec::new(),
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
                        signal_effectors: Vec::new(),
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
                signal_effectors: Vec::new(),
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
        assert_eq!(pheno.sensor_map.len(), 7, "revolute → 1 joint angle + 6 photosensors (2 bodies × 3)");

        // Joint limits should match genotype.
        assert!((pheno.world.joints[0].angle_min[0] - (-0.5)).abs() < 1e-10);
        assert!((pheno.world.joints[0].angle_max[0] - 0.5).abs() < 1e-10);
    }
}
