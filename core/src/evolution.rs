use rand::Rng;

use crate::fitness::{evaluate_swimming_fitness, FitnessConfig};
use crate::genotype::GenomeGraph;
use crate::mating;
use crate::mutation;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct EvolutionConfig {
    pub population_size: usize,
    pub survival_ratio: f64,
    pub asexual_ratio: f64,
    pub crossover_ratio: f64,
    pub grafting_ratio: f64,
    pub fitness: FitnessConfig,
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            population_size: 300,
            survival_ratio: 0.2,
            asexual_ratio: 0.4,
            crossover_ratio: 0.3,
            grafting_ratio: 0.3,
            fitness: FitnessConfig::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Individual & Stats
// ---------------------------------------------------------------------------

pub struct Individual {
    pub genome: GenomeGraph,
    pub fitness: Option<f64>,
}

pub struct GenerationStats {
    pub generation: usize,
    pub best_fitness: f64,
    pub avg_fitness: f64,
    pub worst_fitness: f64,
}

// ---------------------------------------------------------------------------
// Population
// ---------------------------------------------------------------------------

pub struct Population {
    pub individuals: Vec<Individual>,
    pub generation: usize,
    pub config: EvolutionConfig,
    pub stats_history: Vec<GenerationStats>,
}

impl Population {
    /// Create initial population with random genomes.
    pub fn random_initial<R: Rng>(config: EvolutionConfig, rng: &mut R) -> Self {
        let individuals: Vec<Individual> = (0..config.population_size)
            .map(|_| Individual {
                genome: GenomeGraph::random(rng),
                fitness: None,
            })
            .collect();
        Self {
            individuals,
            generation: 0,
            config,
            stats_history: vec![],
        }
    }

    /// Evaluate fitness for all unevaluated individuals.
    pub fn evaluate_all(&mut self) {
        for ind in &mut self.individuals {
            if ind.fitness.is_none() {
                let result = evaluate_swimming_fitness(&ind.genome, &self.config.fitness);
                ind.fitness = Some(result.score);
            }
        }
    }

    /// Run one generation: evaluate → select → reproduce.
    pub fn evolve_generation<R: Rng>(&mut self, rng: &mut R) {
        self.evaluate_all();

        // Sort by fitness descending (NaN-safe)
        self.individuals.sort_by(|a, b| {
            let fa = a.fitness.unwrap_or(0.0);
            let fb = b.fitness.unwrap_or(0.0);
            fb.partial_cmp(&fa).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Record stats
        let fitnesses: Vec<f64> = self
            .individuals
            .iter()
            .map(|i| i.fitness.unwrap_or(0.0))
            .collect();
        self.stats_history.push(GenerationStats {
            generation: self.generation,
            best_fitness: fitnesses[0],
            avg_fitness: fitnesses.iter().sum::<f64>() / fitnesses.len() as f64,
            worst_fitness: *fitnesses.last().unwrap_or(&0.0),
        });

        // Select survivors
        let num_survivors =
            (self.config.population_size as f64 * self.config.survival_ratio).ceil() as usize;
        let survivors: Vec<GenomeGraph> = self.individuals
            [..num_survivors.min(self.individuals.len())]
            .iter()
            .map(|i| i.genome.clone())
            .collect();

        if survivors.is_empty() {
            // All zero fitness — regenerate random
            *self = Population::random_initial(self.config.clone(), rng);
            return;
        }

        // Collect survivor fitnesses for weighted selection
        let survivor_fitnesses: Vec<f64> = self.individuals[..survivors.len()]
            .iter()
            .map(|i| i.fitness.unwrap_or(0.0))
            .collect();

        // Generate new population
        let mut new_pop = Vec::with_capacity(self.config.population_size);

        // Keep survivors (re-evaluate next generation)
        for genome in &survivors {
            new_pop.push(Individual {
                genome: genome.clone(),
                fitness: None,
            });
        }

        // Generate offspring to fill population
        while new_pop.len() < self.config.population_size {
            let roll: f64 = rng.r#gen();
            let offspring_genome = if roll < self.config.asexual_ratio {
                // Asexual: copy + mutate
                let parent = pick_parent_weighted(&survivors, &survivor_fitnesses, rng);
                let mut child = parent.clone();
                mutation::mutate(&mut child, rng);
                child
            } else if roll < self.config.asexual_ratio + self.config.crossover_ratio {
                // Crossover
                let p1 = pick_parent_weighted(&survivors, &survivor_fitnesses, rng);
                let p2 = pick_parent_weighted(&survivors, &survivor_fitnesses, rng);
                let mut child = mating::crossover(p1, p2, rng);
                mutation::mutate(&mut child, rng);
                child
            } else {
                // Grafting
                let p1 = pick_parent_weighted(&survivors, &survivor_fitnesses, rng);
                let p2 = pick_parent_weighted(&survivors, &survivor_fitnesses, rng);
                let mut child = mating::graft(p1, p2, rng);
                mutation::mutate(&mut child, rng);
                child
            };
            new_pop.push(Individual {
                genome: offspring_genome,
                fitness: None,
            });
        }

        self.individuals = new_pop;
        self.generation += 1;
    }

    pub fn best(&self) -> Option<&Individual> {
        self.individuals.iter().max_by(|a, b| {
            let fa = a.fitness.unwrap_or(0.0);
            let fb = b.fitness.unwrap_or(0.0);
            fa.partial_cmp(&fb).unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

/// Pick a parent weighted by fitness (roulette wheel selection).
fn pick_parent_weighted<'a, R: Rng>(
    parents: &'a [GenomeGraph],
    fitnesses: &[f64],
    rng: &mut R,
) -> &'a GenomeGraph {
    let total: f64 = fitnesses.iter().map(|f| f.max(0.001)).sum();
    let mut pick = rng.r#gen::<f64>() * total;
    for (i, &f) in fitnesses.iter().enumerate() {
        pick -= f.max(0.001);
        if pick <= 0.0 {
            return &parents[i];
        }
    }
    &parents[parents.len() - 1]
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
    fn initial_population_correct_size() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let config = EvolutionConfig {
            population_size: 10,
            ..Default::default()
        };
        let pop = Population::random_initial(config, &mut rng);
        assert_eq!(pop.individuals.len(), 10);
    }

    #[test]
    fn generation_maintains_population_size() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let config = EvolutionConfig {
            population_size: 10,
            fitness: FitnessConfig {
                sim_duration: 1.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pop = Population::random_initial(config, &mut rng);
        pop.evolve_generation(&mut rng);
        assert_eq!(pop.individuals.len(), 10);
    }

    #[test]
    fn evolution_runs_multiple_generations() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let config = EvolutionConfig {
            population_size: 10,
            fitness: FitnessConfig {
                sim_duration: 1.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pop = Population::random_initial(config, &mut rng);
        for _ in 0..3 {
            pop.evolve_generation(&mut rng);
        }
        assert_eq!(pop.generation, 3);
        assert_eq!(pop.stats_history.len(), 3);
    }
}
