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
    /// Maximum torque (proportional to cross-sectional area).
    max_torque: f64,
}

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

                // Max torque proportional to min cross-sectional area of connected parts.
                let parent_he = phenotype.world.bodies[joint.parent_idx].half_extents;
                let child_he = phenotype.world.bodies[joint.child_idx].half_extents;
                let parent_area = face_area_min(parent_he);
                let child_area = face_area_min(child_he);
                let max_torque = parent_area.min(child_area) * 10.0;

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
                        RemappedInput::Neuron(idx) => self.prev_outputs[*idx],
                        RemappedInput::Sensor(idx) => self.sensors[*idx],
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
                RemappedInput::Neuron(idx) => self.outputs[*idx],
                RemappedInput::Sensor(idx) => self.sensors[*idx],
                RemappedInput::Constant(v) => *v,
            };
            let torque = (val * eff.weight).clamp(-eff.max_torque, eff.max_torque);
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

/// Minimum face area of a box given half-extents.
/// Returns the area of the smallest face.
fn face_area_min(he: glam::DVec3) -> f64 {
    let xy = 4.0 * he.x * he.y;
    let xz = 4.0 * he.x * he.z;
    let yz = 4.0 * he.y * he.z;
    xy.min(xz).min(yz)
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
}
