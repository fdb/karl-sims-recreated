use crate::brain::BrainInstance;
use crate::genotype::GenomeGraph;
use crate::phenotype::{self, SensorInfo};
use crate::world::World;

pub struct Creature {
    pub genome: GenomeGraph,
    pub world: World,
    pub brain: BrainInstance,
    pub sensor_map: Vec<SensorInfo>,
    pub num_effectors: usize,
}

impl Creature {
    pub fn from_genome(genome: GenomeGraph) -> Self {
        let pheno = phenotype::develop(&genome);
        let brain = BrainInstance::from_phenotype(&genome, &pheno);
        let sensor_map = pheno.sensor_map;
        let num_effectors = pheno.num_effectors;
        Self {
            genome,
            world: pheno.world,
            brain,
            sensor_map,
            num_effectors,
        }
    }

    /// Full-accuracy step (RK45). Used for fitness evaluation.
    pub fn step(&mut self, dt: f64) {
        self.brain.tick(&mut self.world, &self.sensor_map, dt);
        self.world.step(dt);
    }

    /// Fast step (single Euler). Used for browser preview rendering.
    pub fn step_fast(&mut self, dt: f64) {
        self.brain.tick(&mut self.world, &self.sensor_map, dt);
        self.world.step_fast(dt);
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
}
