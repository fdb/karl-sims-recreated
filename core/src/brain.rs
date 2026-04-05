use crate::genotype::{GenomeGraph, NeuronFunc, NeuronInput};
use crate::phenotype::{Phenotype, SensorInfo, SensorType};
use crate::world::World;

// ---------------------------------------------------------------------------
// BrainInstance
// ---------------------------------------------------------------------------

pub struct BrainInstance {
    pub outputs: Vec<f64>,
    pub prev_outputs: Vec<f64>,
    neurons: Vec<NeuronEntry>,
    effectors: Vec<EffectorEntry>,
    sensors: Vec<f64>,
    time: f64,
}

/// A neuron in the flat brain, with inputs remapped to global indices.
#[derive(Debug, Clone)]
struct NeuronEntry {
    func: NeuronFunc,
    inputs: Vec<(RemappedInput, f64)>,
}

/// An effector in the flat brain, with input remapped.
#[derive(Debug, Clone)]
struct EffectorEntry {
    input: RemappedInput,
    weight: f64,
    /// Index of the joint this effector drives.
    joint_idx: usize,
    /// DOF index within the joint.
    dof: usize,
    /// Maximum torque budget for this DOF.
    ///
    /// Sims 1994 §3.2: "The strength of each effector output function is
    /// scaled relative to the maximum cross-sectional dimension of the object
    /// to which it is attached." The effector is conceptually attached to the
    /// child body (it rotates the child relative to the parent), so we scale
    /// by the longest edge of the child box.
    max_torque: f64,
}

/// Torque budget per meter of limb length (N·m / m).
///
/// Calibrated so a canonical 0.3 m cubic limb gets ≈3.6 N·m — the value the
/// previous min-area×10 formula produced for cubes. Elongated limbs get a
/// larger budget than under the old formula, matching the paper's "larger
/// objects move with proportionally larger forces" rationale.
const TORQUE_PER_METER: f64 = 6.0;

#[derive(Debug, Clone)]
enum RemappedInput {
    Neuron(usize),
    Sensor(usize),
    Constant(f64),
}

impl BrainInstance {
    /// Build a BrainInstance from a genome and its developed phenotype.
    ///
    /// Collects neurons and effectors from each body part's brain graph,
    /// remapping local neuron indices to global flat indices.
    pub fn from_phenotype(genome: &GenomeGraph, phenotype: &Phenotype) -> Self {
        let mut neurons: Vec<NeuronEntry> = Vec::new();
        let mut effectors: Vec<EffectorEntry> = Vec::new();

        // For each body in the phenotype, collect its brain.
        // We need to know the neuron offset per body part.
        let mut neuron_offsets: Vec<usize> = Vec::with_capacity(phenotype.body_node_map.len());

        for (body_idx, &(geno_node_idx, _depth)) in
            phenotype.body_node_map.iter().enumerate()
        {
            let brain = &genome.nodes[geno_node_idx].brain;
            let offset = neurons.len();
            neuron_offsets.push(offset);

            // Add neurons, remapping inputs.
            for node in &brain.neurons {
                let inputs: Vec<(RemappedInput, f64)> = node
                    .inputs
                    .iter()
                    .map(|(inp, w)| {
                        let remapped = match inp {
                            NeuronInput::Neuron(idx) => RemappedInput::Neuron(offset + idx),
                            NeuronInput::Sensor(idx) => RemappedInput::Sensor(*idx),
                            NeuronInput::Constant(v) => RemappedInput::Constant(*v),
                        };
                        (remapped, *w)
                    })
                    .collect();

                neurons.push(NeuronEntry {
                    func: node.func,
                    inputs,
                });
            }

            // Find joints where this body is the child, to map effectors to DOFs.
            // Each effector maps to a DOF of the joint connecting this body.
            let mut body_joint_dofs: Vec<(usize, usize)> = Vec::new(); // (joint_idx, dof)
            for (ji, joint) in phenotype.world.joints.iter().enumerate() {
                if joint.child_idx == body_idx {
                    for dof in 0..joint.joint_type.dof_count() {
                        body_joint_dofs.push((ji, dof));
                    }
                }
            }

            // Compute max torque for each joint DOF.
            for (eff_idx, eff) in brain.effectors.iter().enumerate() {
                if eff_idx >= body_joint_dofs.len() {
                    break; // more effectors than DOFs — ignore extras
                }

                let (joint_idx, dof) = body_joint_dofs[eff_idx];
                let joint = &phenotype.world.joints[joint_idx];

                // Paper §3.2: torque budget scales with the maximum
                // cross-sectional dimension (longest edge) of the attached
                // body — here, the child that the effector moves.
                let child_he = phenotype.world.bodies[joint.child_idx].half_extents;
                let max_dim = (child_he.x.max(child_he.y).max(child_he.z)) * 2.0;
                let max_torque = max_dim * TORQUE_PER_METER;

                let remapped_input = match &eff.input {
                    NeuronInput::Neuron(idx) => RemappedInput::Neuron(offset + idx),
                    NeuronInput::Sensor(idx) => RemappedInput::Sensor(*idx),
                    NeuronInput::Constant(v) => RemappedInput::Constant(*v),
                };

                effectors.push(EffectorEntry {
                    input: remapped_input,
                    weight: eff.weight,
                    joint_idx,
                    dof,
                    max_torque,
                });
            }
        }

        // Also add global brain neurons (offset after all body neurons).
        let global_offset = neurons.len();
        for node in &genome.global_brain.neurons {
            let inputs: Vec<(RemappedInput, f64)> = node
                .inputs
                .iter()
                .map(|(inp, w)| {
                    let remapped = match inp {
                        NeuronInput::Neuron(idx) => RemappedInput::Neuron(global_offset + idx),
                        NeuronInput::Sensor(idx) => RemappedInput::Sensor(*idx),
                        NeuronInput::Constant(v) => RemappedInput::Constant(*v),
                    };
                    (remapped, *w)
                })
                .collect();
            neurons.push(NeuronEntry {
                func: node.func,
                inputs,
            });
        }

        let num_neurons = neurons.len();
        let num_sensors = phenotype.sensor_map.len();

        BrainInstance {
            outputs: vec![0.0; num_neurons],
            prev_outputs: vec![0.0; num_neurons],
            neurons,
            effectors,
            sensors: vec![0.0; num_sensors],
            time: 0.0,
        }
    }

    /// Read joint angle sensors from the world.
    pub fn read_sensors(&mut self, world: &World, sensor_map: &[SensorInfo]) {
        for (i, sensor) in sensor_map.iter().enumerate() {
            if i >= self.sensors.len() {
                break;
            }
            match sensor.sensor_type {
                SensorType::JointAngle { joint_idx, dof } => {
                    self.sensors[i] = world.joints[joint_idx].angles[dof];
                }
                SensorType::PhotoSensor { body_idx, axis } => {
                    let body_pos = world.transforms[body_idx].translation;
                    let light_dir_world = (world.light_position - body_pos).normalize_or_zero();
                    let body_rot_inv = world.transforms[body_idx].matrix3.transpose();
                    let light_dir_local = body_rot_inv * light_dir_world;
                    self.sensors[i] = match axis {
                        0 => light_dir_local.x,
                        1 => light_dir_local.y,
                        2 => light_dir_local.z,
                        _ => 0.0,
                    };
                }
            }
        }
    }

    /// Evaluate one brain step: compute all neuron outputs from current inputs.
    pub fn evaluate_step(&mut self) {
        // Swap outputs and prev_outputs.
        std::mem::swap(&mut self.outputs, &mut self.prev_outputs);

        for i in 0..self.neurons.len() {
            let neuron = &self.neurons[i];
            let func = neuron.func;

            // Compute weighted input values.
            let weighted_inputs: Vec<f64> = neuron
                .inputs
                .iter()
                .map(|(inp, weight)| {
                    let val = match inp {
                        RemappedInput::Neuron(idx) => {
                            self.prev_outputs.get(*idx).copied().unwrap_or(0.0)
                        }
                        RemappedInput::Sensor(idx) => {
                            self.sensors.get(*idx).copied().unwrap_or(0.0)
                        }
                        RemappedInput::Constant(v) => *v,
                    };
                    val * weight
                })
                .collect();

            let result = match func {
                NeuronFunc::Sum => weighted_inputs.iter().sum(),
                NeuronFunc::Product => {
                    if weighted_inputs.is_empty() {
                        0.0
                    } else {
                        weighted_inputs.iter().copied().fold(1.0, |a, b| a * b)
                    }
                }
                NeuronFunc::Sigmoid => {
                    let sum: f64 = weighted_inputs.iter().sum();
                    1.0 / (1.0 + (-sum).exp())
                }
                NeuronFunc::Sin => {
                    let sum: f64 = weighted_inputs.iter().sum();
                    sum.sin()
                }
                NeuronFunc::OscillateWave => {
                    // sin(time * freq + phase)
                    // freq = input[0], phase = input[1]
                    let freq = weighted_inputs.first().copied().unwrap_or(1.0);
                    let phase = weighted_inputs.get(1).copied().unwrap_or(0.0);
                    (self.time * freq + phase).sin()
                }
                NeuronFunc::Memory => {
                    let sum: f64 = weighted_inputs.iter().sum();
                    0.5 * self.prev_outputs[i] + 0.5 * sum
                }
            };

            self.outputs[i] = result;
        }
    }

    /// Apply effector outputs as torques to joints in the world.
    pub fn apply_effectors(&self, world: &mut World) {
        // Zero all torques first.
        for torque in world.torques.iter_mut() {
            *torque = [0.0; 3];
        }

        for eff in &self.effectors {
            let val = match &eff.input {
                RemappedInput::Neuron(idx) => self.outputs.get(*idx).copied().unwrap_or(0.0),
                RemappedInput::Sensor(idx) => self.sensors.get(*idx).copied().unwrap_or(0.0),
                RemappedInput::Constant(v) => *v,
            };
            // (val * weight) is the commanded fraction of the motion budget,
            // clamped to [-1, 1]. Then we scale by the physical torque limit.
            // This ensures torque ≤ max_torque regardless of neuron output
            // magnitude, and gives graded control: a command of 0.5 produces
            // half-torque, not saturation. (Sims 1994 §3.2.)
            let command = (val * eff.weight).clamp(-1.0, 1.0);
            let torque = command * eff.max_torque;
            world.torques[eff.joint_idx][eff.dof] = torque;
        }
    }

    /// Advance brain by one physics timestep.
    ///
    /// Runs 2 brain evaluation steps per physics step (per the Sims paper)
    /// to reduce signal propagation delay.
    pub fn tick(&mut self, world: &mut World, sensor_map: &[SensorInfo], dt: f64) {
        self.time += dt;
        self.read_sensors(world, sensor_map);

        // Two brain steps per physics step.
        self.evaluate_step();
        self.evaluate_step();

        self.apply_effectors(world);
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
    use crate::phenotype::develop;
    use glam::DVec3;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn brain_from_random_genome() {
        for seed in 0..20u64 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let genome = GenomeGraph::random(&mut rng);
            let pheno = develop(&genome);
            let _brain = BrainInstance::from_phenotype(&genome, &pheno);
            // Should not panic.
        }
    }

    #[test]
    fn brain_tick_produces_torques() {
        // Find a seed that produces joints.
        for seed in 0..100u64 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let genome = GenomeGraph::random(&mut rng);
            let mut pheno = develop(&genome);

            if pheno.world.joints.is_empty() {
                continue;
            }

            let sensor_map = pheno.sensor_map.clone();
            let mut brain = BrainInstance::from_phenotype(&genome, &pheno);

            // Tick a few times.
            let dt = 1.0 / 60.0;
            for _ in 0..10 {
                brain.tick(&mut pheno.world, &sensor_map, dt);
            }

            // Check that at least some torques are non-zero.
            let any_nonzero = pheno
                .world
                .torques
                .iter()
                .any(|t| t.iter().any(|&v| v.abs() > 1e-12));

            if any_nonzero {
                // Test passes: we found a seed that produces torques.
                return;
            }
        }

        panic!("No seed in 0..100 produced non-zero torques");
    }

    #[test]
    fn oscillate_wave_produces_varying_output() {
        // Build a minimal genome: 2 nodes, 1 revolute connection,
        // with an OscillateWave neuron driving the effector.
        let genome = GenomeGraph {
            nodes: vec![
                MorphNode {
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
                },
                MorphNode {
                    dimensions: DVec3::new(0.4, 0.3, 0.3),
                    joint_type: JointType::Revolute,
                    joint_limit_min: [-1.5; 3],
                    joint_limit_max: [1.5; 3],
                    recursive_limit: 1,
                    terminal_only: false,
                    brain: BrainGraph {
                        neurons: vec![BrainNode {
                            func: NeuronFunc::OscillateWave,
                            inputs: vec![
                                (NeuronInput::Constant(3.0), 1.0), // freq
                                (NeuronInput::Constant(0.0), 1.0), // phase
                            ],
                        }],
                        effectors: vec![EffectorNode {
                            input: NeuronInput::Neuron(0),
                            weight: 2.0,
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

        let mut pheno = develop(&genome);
        assert_eq!(pheno.world.joints.len(), 1);

        let sensor_map = pheno.sensor_map.clone();
        let mut brain = BrainInstance::from_phenotype(&genome, &pheno);

        let dt = 1.0 / 60.0;
        let mut torque_values: Vec<f64> = Vec::new();

        for _ in 0..60 {
            brain.tick(&mut pheno.world, &sensor_map, dt);
            torque_values.push(pheno.world.torques[0][0]);
        }

        let max_t = torque_values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let min_t = torque_values.iter().copied().fold(f64::INFINITY, f64::min);

        assert!(
            max_t - min_t > 0.1,
            "Torques should vary over time; range was {}",
            max_t - min_t
        );
    }

    // Build a two-body genome with a single revolute DOF driven by a
    // Constant-input effector of known command value and weight. Returns the
    // developed phenotype and brain, ready to tick.
    fn two_body_with_constant_effector(
        child_dims: DVec3,
        effector_command: f64,
        effector_weight: f64,
    ) -> (Phenotype, BrainInstance) {
        let genome = GenomeGraph {
            nodes: vec![
                MorphNode {
                    dimensions: DVec3::new(0.5, 0.5, 0.5),
                    joint_type: JointType::Rigid,
                    joint_limit_min: [-1.0; 3],
                    joint_limit_max: [1.0; 3],
                    recursive_limit: 1,
                    terminal_only: false,
                    brain: BrainGraph { neurons: Vec::new(), effectors: Vec::new() },
                },
                MorphNode {
                    dimensions: child_dims,
                    joint_type: JointType::Revolute,
                    joint_limit_min: [-1.5; 3],
                    joint_limit_max: [1.5; 3],
                    recursive_limit: 1,
                    terminal_only: false,
                    brain: BrainGraph {
                        neurons: Vec::new(),
                        effectors: vec![EffectorNode {
                            input: NeuronInput::Constant(effector_command),
                            weight: effector_weight,
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
            global_brain: BrainGraph { neurons: Vec::new(), effectors: Vec::new() },
        };
        let pheno = develop(&genome);
        assert_eq!(pheno.world.joints.len(), 1);
        let brain = BrainInstance::from_phenotype(&genome, &pheno);
        (pheno, brain)
    }

    use crate::phenotype::Phenotype;

    #[test]
    fn max_torque_scales_with_child_longest_edge() {
        // Paper §3.2: strength scaled by the max cross-sectional dimension
        // of the attached (child) body. Longest edge of (0.4, 0.3, 0.3) is
        // 0.4 m → max_torque = 0.4 * TORQUE_PER_METER = 2.4 N·m.
        let (mut pheno, brain) = two_body_with_constant_effector(
            DVec3::new(0.4, 0.3, 0.3),
            1.0,  // command = 1.0
            1.0,  // weight = 1.0 → saturates at command of ±1
        );
        brain.apply_effectors(&mut pheno.world);
        let torque = pheno.world.torques[0][0];
        let expected = 0.4 * TORQUE_PER_METER;
        assert!(
            (torque - expected).abs() < 1e-9,
            "expected torque ≈ {expected}, got {torque}"
        );
    }

    #[test]
    fn max_torque_uses_longest_edge_not_min_area() {
        // An elongated limb (0.1, 0.1, 0.6) has longest edge 0.6 m.
        // Under the old min-area formula it would have been
        // min_area*10 = 0.04 * 10 = 0.4. Under the paper formula it is
        // 0.6 * 6.0 = 3.6. Assert we are on the paper side.
        let (mut pheno, brain) = two_body_with_constant_effector(
            DVec3::new(0.1, 0.1, 0.6),
            1.0, 1.0,
        );
        brain.apply_effectors(&mut pheno.world);
        let torque = pheno.world.torques[0][0];
        assert!(
            torque > 3.0,
            "elongated limb should have substantial torque budget under paper \
             formula, got {torque}"
        );
    }

    #[test]
    fn torque_never_exceeds_budget_for_huge_command() {
        // A neuron output of 1000 with weight 1.0 should still produce torque
        // ≤ max_torque — this is the motion-budget guarantee.
        let (mut pheno, brain) = two_body_with_constant_effector(
            DVec3::new(0.3, 0.3, 0.3),
            1000.0, 1.0,
        );
        brain.apply_effectors(&mut pheno.world);
        let torque = pheno.world.torques[0][0];
        let max_torque = 0.3 * TORQUE_PER_METER;
        assert!(
            torque.abs() <= max_torque + 1e-9,
            "torque {torque} exceeded max_torque {max_torque}"
        );
        assert!(
            (torque - max_torque).abs() < 1e-9,
            "saturated torque should equal max_torque exactly, got {torque}"
        );
    }

    #[test]
    fn graded_torque_scales_linearly_with_command() {
        // A command of 0.5 with weight 1.0 should produce half-budget torque,
        // not saturated torque. This is the "graded control" property.
        let (mut pheno, brain) = two_body_with_constant_effector(
            DVec3::new(0.3, 0.3, 0.3),
            0.5, 1.0,
        );
        brain.apply_effectors(&mut pheno.world);
        let torque = pheno.world.torques[0][0];
        let max_torque = 0.3 * TORQUE_PER_METER;
        let expected = 0.5 * max_torque;
        assert!(
            (torque - expected).abs() < 1e-9,
            "expected graded torque {expected}, got {torque}"
        );
    }
}
