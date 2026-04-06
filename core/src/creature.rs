use crate::brain::BrainInstance;
use crate::genotype::GenomeGraph;
use crate::phenotype::{self, GrowthPlan, SensorInfo};
use crate::world::World;

pub struct Creature {
    pub genome: GenomeGraph,
    pub world: World,
    pub brain: BrainInstance,
    pub sensor_map: Vec<SensorInfo>,
    pub num_effectors: usize,
    /// Optional growth plan for developmental growth. `None` means
    /// the creature was fully developed at instantiation time.
    growth_plan: Option<GrowthPlan>,
    /// Frames between growth events.
    growth_interval: usize,
    /// Frame counter for growth timing.
    frame_count: usize,
    /// Number of signal channels (needed for brain rebuilds during growth).
    num_signal_channels: usize,
    /// (genotype node index, depth) for each body — mirrors phenotype.body_node_map
    /// but kept live for growth rebuilds.
    pub body_node_map: Vec<(usize, u32)>,
}

impl Creature {
    pub fn from_genome(genome: GenomeGraph) -> Self {
        Self::from_genome_with_signals(genome, 0)
    }

    /// Create a creature with inter-body broadcast signal channels.
    pub fn from_genome_with_signals(genome: GenomeGraph, num_signal_channels: usize) -> Self {
        let pheno = phenotype::develop(&genome);
        let brain = BrainInstance::from_phenotype_with_signals(&genome, &pheno, num_signal_channels);
        let sensor_map = pheno.sensor_map;
        let num_effectors = pheno.num_effectors;
        let body_node_map = pheno.body_node_map;
        Self {
            genome,
            world: pheno.world,
            brain,
            sensor_map,
            num_effectors,
            growth_plan: None,
            growth_interval: 0,
            frame_count: 0,
            num_signal_channels,
            body_node_map,
        }
    }

    /// Create a creature with developmental growth enabled.
    ///
    /// The creature starts with only the root body and grows one segment
    /// every `growth_interval` frames until fully developed.
    pub fn from_genome_with_growth(
        genome: GenomeGraph,
        num_signal_channels: usize,
        growth_interval: usize,
    ) -> Self {
        let (pheno, plan) = phenotype::develop_with_growth_plan(&genome);
        let brain = BrainInstance::from_phenotype_with_signals(&genome, &pheno, num_signal_channels);
        let sensor_map = pheno.sensor_map;
        let num_effectors = pheno.num_effectors;
        let body_node_map = pheno.body_node_map;

        let growth_plan = if plan.steps.is_empty() {
            None
        } else {
            Some(plan)
        };

        Self {
            genome,
            world: pheno.world,
            brain,
            sensor_map,
            num_effectors,
            growth_plan,
            growth_interval,
            frame_count: 0,
            num_signal_channels,
            body_node_map,
        }
    }

    /// Step brain and physics forward by `dt` seconds.
    pub fn step(&mut self, dt: f64) {
        self.frame_count += 1;

        // Check if it's time to grow a new body segment.
        if let Some(ref mut plan) = self.growth_plan {
            if self.growth_interval > 0
                && self.frame_count % self.growth_interval == 0
                && plan.next_step < plan.steps.len()
            {
                let step = plan.steps[plan.next_step].clone();
                let new_body_idx = phenotype::grow_one_step(
                    &self.genome,
                    &mut self.world,
                    &mut self.body_node_map,
                    &mut self.sensor_map,
                    &step,
                );

                // Update parent_body_idx for future growth steps that depend on this body.
                // The growth plan was built with virtual body indices; now we have the real one.
                let expected_virtual = self.world.root + plan.next_step + 1;
                for future_step in plan.steps[plan.next_step + 1..].iter_mut() {
                    if future_step.parent_body_idx == expected_virtual {
                        future_step.parent_body_idx = new_body_idx;
                    }
                }

                plan.next_step += 1;

                // Rebuild the brain to include the new body's neurons/effectors.
                // We create a temporary Phenotype-like view to pass to from_phenotype_with_signals.
                let temp_pheno = phenotype::Phenotype {
                    world: self.world.clone(),
                    body_node_map: self.body_node_map.clone(),
                    num_effectors: self.body_node_map.iter()
                        .map(|(ni, _)| self.genome.nodes[*ni].brain.effectors.len())
                        .sum(),
                    sensor_map: self.sensor_map.clone(),
                };
                self.brain = BrainInstance::from_phenotype_with_signals(
                    &self.genome,
                    &temp_pheno,
                    self.num_signal_channels,
                );
                self.num_effectors = temp_pheno.num_effectors;

                // If all steps done, drop the plan.
                if plan.next_step >= plan.steps.len() {
                    self.growth_plan = None;
                }
            }
        }

        self.brain.tick(&mut self.world, &self.sensor_map, dt);
        self.world.step(dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn creature_from_random_genome() {
        for seed in 0..20u64 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let genome = GenomeGraph::random(&mut rng);
            let creature = Creature::from_genome(genome);
            assert!(!creature.world.bodies.is_empty());
        }
    }

    #[test]
    fn creature_step_runs_without_panic() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let genome = GenomeGraph::random(&mut rng);
        let mut creature = Creature::from_genome(genome);
        for _ in 0..60 {
            creature.step(1.0 / 60.0);
        }
    }

    #[test]
    fn creature_with_growth_eventually_has_all_bodies() {
        // Find a seed that produces a multi-body creature.
        for seed in 0..50u64 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let genome = GenomeGraph::random(&mut rng);

            // Check how many bodies the fully-developed creature has.
            let full = Creature::from_genome(genome.clone());
            let expected_bodies = full.world.bodies.len();
            if expected_bodies <= 1 {
                continue; // Single-body, not interesting for growth test
            }

            // Create with growth (1 body per frame for fast testing).
            let mut growing = Creature::from_genome_with_growth(genome, 0, 1);
            assert_eq!(growing.world.bodies.len(), 1, "seed {seed}: should start with root only");

            // Step enough frames to grow all bodies.
            let dt = 1.0 / 60.0;
            for _ in 0..(expected_bodies * 2) {
                growing.step(dt);
            }

            assert_eq!(
                growing.world.bodies.len(),
                expected_bodies,
                "seed {seed}: growth should produce same number of bodies as instant develop"
            );
            return; // Found a working seed, test passes.
        }
        panic!("No multi-body seed found in 0..50");
    }

    #[test]
    fn creature_with_signals_runs_without_panic() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let genome = GenomeGraph::random(&mut rng);
        let mut creature = Creature::from_genome_with_signals(genome, 4);
        for _ in 0..60 {
            creature.step(1.0 / 60.0);
        }
    }
}
