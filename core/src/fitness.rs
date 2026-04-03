use crate::creature::Creature;
use crate::genotype::GenomeGraph;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FitnessConfig {
    pub sim_duration: f64,
    pub dt: f64,
    pub max_parts: usize,
    pub early_termination_time: f64,
    pub min_movement: f64,
}

impl Default for FitnessConfig {
    fn default() -> Self {
        Self {
            sim_duration: 10.0,
            dt: 1.0 / 60.0,
            max_parts: 20,
            early_termination_time: 2.0,
            min_movement: 0.01,
        }
    }
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

pub struct FitnessResult {
    pub score: f64,
    pub distance: f64,
    pub max_displacement: f64,
    pub terminated_early: bool,
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

pub fn evaluate_swimming_fitness(
    genome: &GenomeGraph,
    config: &FitnessConfig,
) -> FitnessResult {
    let mut creature = Creature::from_genome(genome.clone());

    // Viability check: too many parts → zero fitness.
    if creature.world.bodies.len() > config.max_parts {
        return FitnessResult {
            score: 0.0,
            distance: 0.0,
            max_displacement: 0.0,
            terminated_early: false,
        };
    }

    let initial_pos = creature.world.transforms[creature.world.root].translation;
    let total_steps = (config.sim_duration / config.dt).round() as usize;
    let early_check_step = (config.early_termination_time / config.dt).round() as usize;

    let mut max_displacement: f64 = 0.0;
    let mut terminated_early = false;

    for step in 0..total_steps {
        creature.step(config.dt);

        let current_pos = creature.world.transforms[creature.world.root].translation;
        let disp = (current_pos - initial_pos).length();
        if disp > max_displacement {
            max_displacement = disp;
        }

        // Early termination check.
        if step + 1 == early_check_step && disp < config.min_movement {
            terminated_early = true;
            break;
        }
    }

    let final_pos = creature.world.transforms[creature.world.root].translation;
    let distance = (final_pos - initial_pos).length();

    let score = if terminated_early {
        0.0
    } else {
        distance * 0.7 + max_displacement * 0.3
    };

    FitnessResult {
        score,
        distance,
        max_displacement,
        terminated_early,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genotype::{
        AttachFace, BrainGraph, GenomeGraph, MorphConn, MorphNode,
    };
    use crate::joint::JointType;
    use glam::DVec3;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    /// Build a genome that produces many parts via high recursive limits.
    fn large_genome() -> GenomeGraph {
        let mut nodes = Vec::new();
        // Root node
        nodes.push(MorphNode {
            dimensions: DVec3::new(0.3, 0.3, 0.3),
            joint_type: JointType::Rigid,
            joint_limit_min: [-1.0; 3],
            joint_limit_max: [1.0; 3],
            recursive_limit: 8,
            terminal_only: false,
            brain: BrainGraph {
                neurons: Vec::new(),
                effectors: Vec::new(),
            },
        });
        // Child node with high recursion
        nodes.push(MorphNode {
            dimensions: DVec3::new(0.2, 0.2, 0.2),
            joint_type: JointType::Revolute,
            joint_limit_min: [-1.0; 3],
            joint_limit_max: [1.0; 3],
            recursive_limit: 8,
            terminal_only: false,
            brain: BrainGraph::random_for_joint(
                &mut ChaCha8Rng::seed_from_u64(0),
                JointType::Revolute,
            ),
        });

        let connections = vec![
            MorphConn {
                source: 0,
                target: 1,
                parent_face: AttachFace::PosX,
                child_face: AttachFace::NegX,
                scale: 1.0,
                reflection: true,
            },
            // Self-referencing connection to drive recursion
            MorphConn {
                source: 1,
                target: 1,
                parent_face: AttachFace::PosX,
                child_face: AttachFace::NegX,
                scale: 0.9,
                reflection: false,
            },
        ];

        GenomeGraph {
            nodes,
            connections,
            root: 0,
            global_brain: BrainGraph {
                neurons: Vec::new(),
                effectors: Vec::new(),
            },
        }
    }

    #[test]
    fn large_creature_gets_zero_fitness() {
        let genome = large_genome();
        let config = FitnessConfig {
            max_parts: 5,
            ..Default::default()
        };
        let result = evaluate_swimming_fitness(&genome, &config);
        assert_eq!(result.score, 0.0, "creature with too many parts should get zero fitness");
    }

    #[test]
    fn fitness_evaluation_completes() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let genome = GenomeGraph::random(&mut rng);
        let config = FitnessConfig {
            sim_duration: 1.0, // short for test speed
            ..Default::default()
        };
        let result = evaluate_swimming_fitness(&genome, &config);
        // Should complete without panic; score is non-negative.
        assert!(result.score >= 0.0);
        assert!(result.distance >= 0.0);
        assert!(result.max_displacement >= 0.0);
    }

    #[test]
    fn fitness_is_deterministic() {
        let mut rng = ChaCha8Rng::seed_from_u64(77);
        let genome = GenomeGraph::random(&mut rng);
        let config = FitnessConfig {
            sim_duration: 1.0,
            ..Default::default()
        };

        let r1 = evaluate_swimming_fitness(&genome, &config);
        let r2 = evaluate_swimming_fitness(&genome, &config);

        assert_eq!(
            r1.score, r2.score,
            "same genome should produce identical fitness"
        );
        assert_eq!(r1.distance, r2.distance);
        assert_eq!(r1.max_displacement, r2.max_displacement);
        assert_eq!(r1.terminated_early, r2.terminated_early);
    }
}
