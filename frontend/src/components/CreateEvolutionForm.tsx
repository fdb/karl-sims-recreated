import { useState } from "react";
import { createEvolution } from "../api";

interface Props {
  onCreated: () => void;
  onCancel: () => void;
}

export default function CreateEvolutionForm({ onCreated, onCancel }: Props) {
  const [goal, setGoal] = useState("swimming_speed");
  const [environment, setEnvironment] = useState("land");
  const [popSize, setPopSize] = useState(300);
  const [maxGen, setMaxGen] = useState(300);
  const [simDuration, setSimDuration] = useState(10);
  const [maxParts, setMaxParts] = useState(5);
  const [gravity, setGravity] = useState(9.81);
  const [viscosity, setViscosity] = useState(2.0);
  const [numIslands, setNumIslands] = useState(5);
  const [migrationInterval, setMigrationInterval] = useState(0);
  const [islandStrategy, setIslandStrategy] = useState("isolated");
  const [exchangeInterval, setExchangeInterval] = useState(10);
  const [diversityPressure, setDiversityPressure] = useState(0.0);
  const [airtimePenalty, setAirtimePenalty] = useState(0.0);
  const [minJointMotion, setMinJointMotion] = useState(0.2);
  const [maxJointAngularVelocity, setMaxJointAngularVelocity] = useState(20);
  const [solverIterations, setSolverIterations] = useState(8);
  const [pgsIterations, setPgsIterations] = useState(2);
  const [frictionCoefficient, setFrictionCoefficient] = useState(1.5);
  const [useCoulombFriction, setUseCoulombFriction] = useState(true);
  const [frictionCombineMax, setFrictionCombineMax] = useState(true);
  const [name, setName] = useState("");
  const [creating, setCreating] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [showSolver, setShowSolver] = useState(false);

  const handleSubmit = async () => {
    setCreating(true);
    await createEvolution({
      population_size: popSize,
      generations: maxGen,
      goal,
      environment,
      sim_duration: simDuration,
      max_parts: maxParts,
      gravity: environment === "land" ? gravity : undefined,
      water_viscosity: environment === "water" ? viscosity : undefined,
      num_islands: numIslands,
      migration_interval: migrationInterval,
      min_joint_motion: minJointMotion,
      max_joint_angular_velocity: maxJointAngularVelocity,
      solver_iterations: solverIterations,
      pgs_iterations: pgsIterations,
      friction_coefficient: frictionCoefficient,
      use_coulomb_friction: useCoulombFriction,
      friction_combine_max: frictionCombineMax,
      airtime_penalty: airtimePenalty > 0 ? airtimePenalty : undefined,
      island_strategy: islandStrategy,
      exchange_interval: islandStrategy === "hfc" ? exchangeInterval : undefined,
      migration_interval: islandStrategy === "ring_migration" ? migrationInterval : 0,
      diversity_pressure: diversityPressure > 0 ? diversityPressure : undefined,
      name: name.trim() || undefined,
    });
    setCreating(false);
    onCreated();
  };

  const inputClass =
    "w-full px-3 py-1.5 bg-bg-base border border-border rounded-md text-sm text-text-primary focus:outline-none focus:border-accent";
  const labelClass = "block text-xs text-text-secondary mb-1";

  return (
    <div className="bg-bg-surface border border-border rounded-lg p-6 mb-6">
      <h3 className="text-lg font-semibold mb-4">New Evolution Run</h3>

      <div className="grid grid-cols-2 gap-x-8 gap-y-4">
        {/* Name (optional, spans full width) */}
        <div className="col-span-2">
          <label className={labelClass}>Name <span className="text-text-muted">(optional)</span></label>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Land walkers v3"
            maxLength={100}
            className={inputClass}
          />
        </div>

        {/* Environment */}
        <div>
          <label className={labelClass}>Environment</label>
          <select
            value={environment}
            onChange={(e) => setEnvironment(e.target.value)}
            className={inputClass}
          >
            <option value="water">Water (no gravity, viscous drag)</option>
            <option value="land">Land (gravity, ground collision)</option>
          </select>
          <p className="text-xs text-text-muted mt-1">
            {environment === "water"
              ? "Zero gravity, per-face viscous water drag."
              : "Gravity at 9.81 m/s\u00B2, ground plane with collisions."}
          </p>
        </div>

        {/* Goal */}
        <div>
          <label className={labelClass}>Fitness Goal</label>
          <select
            value={goal}
            onChange={(e) => setGoal(e.target.value)}
            className={inputClass}
          >
            <option value="swimming_speed">
              {environment === "land" ? "Locomotion Speed" : "Swimming Speed"}
            </option>
            <option value="light_following">Light Following</option>
          </select>
          <p className="text-xs text-text-muted mt-1">
            {goal === "swimming_speed"
              ? environment === "land"
                ? "Evolve creatures that move the fastest across the ground."
                : "Evolve creatures that swim the fastest in a straight line."
              : "Evolve creatures that follow a moving light source."}
          </p>
        </div>

        {/* Population Size */}
        <div>
          <label className={labelClass}>Population Size</label>
          <input
            type="number"
            value={popSize}
            onChange={(e) => setPopSize(Number(e.target.value))}
            min={10}
            max={1000}
            className={inputClass}
          />
          <p className="text-xs text-text-muted mt-1">
            Sims 1994 used 300. Larger = more diversity.
          </p>
        </div>

        {/* Max Generations */}
        <div>
          <label className={labelClass}>Max Generations</label>
          <input
            type="number"
            value={maxGen}
            onChange={(e) => setMaxGen(Number(e.target.value))}
            min={1}
            max={10000}
            className={inputClass}
          />
        </div>

        {/* Sim Duration */}
        <div>
          <label className={labelClass}>Simulation Duration (seconds)</label>
          <input
            type="number"
            value={simDuration}
            onChange={(e) => setSimDuration(Number(e.target.value))}
            min={1}
            max={60}
            step={1}
            className={inputClass}
          />
        </div>

        {/* Max Parts */}
        <div>
          <label className={labelClass}>Max Body Parts</label>
          <input
            type="number"
            value={maxParts}
            onChange={(e) => setMaxParts(Number(e.target.value))}
            min={2}
            max={50}
            className={inputClass}
          />
        </div>

        {/* Gravity (land only) */}
        {environment === "land" && (
          <div>
            <label className={labelClass}>Gravity (m/s\u00B2)</label>
            <input
              type="number"
              value={gravity}
              onChange={(e) => setGravity(Number(e.target.value))}
              min={0}
              max={30}
              step={0.1}
              className={inputClass}
            />
            <p className="text-xs text-text-muted mt-1">
              Earth: 9.81, Moon: 1.62, Mars: 3.72
            </p>
          </div>
        )}

        {/* Water Viscosity (water only) */}
        {environment === "water" && (
          <div>
            <label className={labelClass}>Water Viscosity</label>
            <input
              type="number"
              value={viscosity}
              onChange={(e) => setViscosity(Number(e.target.value))}
              min={0.1}
              max={10}
              step={0.1}
              className={inputClass}
            />
            <p className="text-xs text-text-muted mt-1">
              Higher = thicker fluid. Default: 2.0
            </p>
          </div>
        )}

        {/* Number of islands */}
        <div>
          <label className={labelClass}>Islands</label>
          <input
            type="number"
            value={numIslands}
            onChange={(e) => setNumIslands(Number(e.target.value))}
            min={1}
            max={12}
            className={inputClass}
          />
          <p className="text-xs text-text-muted mt-1">
            Parallel sub-populations for species diversity. With {numIslands}{" "}
            islands, each gets ~{Math.max(1, Math.floor(popSize / numIslands))}{" "}
            creatures.
          </p>
        </div>

        {/* Island strategy */}
        <div>
          <label className={labelClass}>Island Strategy</label>
          <select
            value={islandStrategy}
            onChange={(e) => setIslandStrategy(e.target.value)}
            className={inputClass}
            disabled={numIslands <= 1}
          >
            <option value="isolated">Isolated (no migration)</option>
            <option value="ring_migration">Ring Migration</option>
            <option value="hfc">HFC (Hierarchical Fair Competition)</option>
          </select>
          <p className="text-xs text-text-muted mt-1">
            {islandStrategy === "isolated"
              ? "Each island evolves independently — different species per island."
              : islandStrategy === "ring_migration"
              ? "Best creature migrates along a ring every N generations."
              : "Islands become fitness tiers. Novel low-fitness creatures get protected from established winners."}
          </p>
        </div>

        {/* Migration interval (ring only) */}
        {islandStrategy === "ring_migration" && (
          <div>
            <label className={labelClass}>Migration Interval (gens)</label>
            <input
              type="number"
              value={migrationInterval}
              onChange={(e) => setMigrationInterval(Number(e.target.value))}
              min={1}
              max={1000}
              className={inputClass}
            />
            <p className="text-xs text-text-muted mt-1">
              Best of each island moves to next along a ring.
            </p>
          </div>
        )}

        {/* Exchange interval (HFC only) */}
        {islandStrategy === "hfc" && (
          <div>
            <label className={labelClass}>Exchange Interval (gens)</label>
            <input
              type="number"
              value={exchangeInterval}
              onChange={(e) => setExchangeInterval(Number(e.target.value))}
              min={1}
              max={100}
              className={inputClass}
            />
            <p className="text-xs text-text-muted mt-1">
              How often creatures are promoted between fitness tiers.
            </p>
          </div>
        )}

        {/* Diversity pressure */}
        <div>
          <label className={labelClass}>Diversity Pressure: {diversityPressure.toFixed(2)}</label>
          <input
            type="range"
            value={diversityPressure}
            onChange={(e) => setDiversityPressure(Number(e.target.value))}
            min={0}
            max={1}
            step={0.05}
            className="w-full accent-accent"
          />
          <p className="text-xs text-text-muted mt-1">
            Morphological niching: penalizes creatures similar to others. 0 = off.
          </p>
        </div>

        {/* Airtime penalty (land only) */}
        {environment === "land" && (
          <div>
            <label className={labelClass}>Airtime Penalty: {airtimePenalty.toFixed(2)}</label>
            <input
              type="range"
              value={airtimePenalty}
              onChange={(e) => setAirtimePenalty(Number(e.target.value))}
              min={0}
              max={1}
              step={0.05}
              className="w-full accent-accent"
            />
            <p className="text-xs text-text-muted mt-1">
              Penalizes hopping/jumping. Pushes toward ground-contact gaits (walking, crawling). 0 = off.
            </p>
          </div>
        )}
      </div>

      {/* Advanced settings toggle */}
      <button
        onClick={() => setShowAdvanced(!showAdvanced)}
        className="mt-4 text-xs text-text-muted hover:text-text-secondary transition-colors"
      >
        {showAdvanced ? "\u25BC" : "\u25B6"} Anti-exploit guards
      </button>

      {showAdvanced && (
        <div className="grid grid-cols-2 gap-x-8 gap-y-4 mt-3 pt-3 border-t border-border">
          {/* Min Joint Motion */}
          <div>
            <label className={labelClass}>Min Joint Motion (rad stddev)</label>
            <input
              type="number"
              value={minJointMotion}
              onChange={(e) => setMinJointMotion(Number(e.target.value))}
              min={0}
              max={2}
              step={0.05}
              className={inputClass}
            />
            <p className="text-xs text-text-muted mt-1">
              Joints must oscillate above this threshold in every 2s window.
              Lower = more permissive. 0 = disabled (paper-faithful).
            </p>
          </div>

          {/* Max Joint Angular Velocity */}
          <div>
            <label className={labelClass}>Max Joint Angular Velocity (rad/s)</label>
            <input
              type="number"
              value={maxJointAngularVelocity}
              onChange={(e) =>
                setMaxJointAngularVelocity(Number(e.target.value))
              }
              min={0}
              max={100}
              step={1}
              className={inputClass}
            />
            <p className="text-xs text-text-muted mt-1">
              Rejects creatures with joints faster than this. Lower = stricter
              (blocks sliding exploits). Too low kills multi-body diversity.
            </p>
          </div>
        </div>
      )}

      {/* Physics solver toggle */}
      <button
        onClick={() => setShowSolver(!showSolver)}
        className="block mt-1 text-xs text-text-muted hover:text-text-secondary transition-colors"
      >
        {showSolver ? "\u25BC" : "\u25B6"} Physics solver
      </button>

      {showSolver && (
        <div className="grid grid-cols-2 gap-x-8 gap-y-4 mt-3 pt-3 border-t border-border">
          {/* Solver Iterations */}
          <div>
            <label className={labelClass}>Solver Iterations</label>
            <input
              type="number"
              value={solverIterations}
              onChange={(e) => setSolverIterations(Number(e.target.value))}
              min={1}
              max={64}
              step={1}
              className={inputClass}
            />
            <p className="text-xs text-text-muted mt-1">
              Rapier default: 4. Higher = better friction convergence, less sliding. 8-16 recommended.
            </p>
          </div>

          {/* PGS Iterations */}
          <div>
            <label className={labelClass}>PGS Iterations</label>
            <input
              type="number"
              value={pgsIterations}
              onChange={(e) => setPgsIterations(Number(e.target.value))}
              min={1}
              max={16}
              step={1}
              className={inputClass}
            />
            <p className="text-xs text-text-muted mt-1">
              Internal passes per solver iteration. Total work = solver {"\u00D7"} PGS.
            </p>
          </div>

          {/* Friction Coefficient */}
          <div>
            <label className={labelClass}>Friction Coefficient</label>
            <input
              type="number"
              value={frictionCoefficient}
              onChange={(e) => setFrictionCoefficient(Number(e.target.value))}
              min={0}
              max={10}
              step={0.1}
              className={inputClass}
            />
            <p className="text-xs text-text-muted mt-1">
              Rapier default: 0.8. Values {">"}1.0 compensate for missing static friction.
            </p>
          </div>

          {/* Checkboxes row */}
          <div className="flex flex-col gap-3 justify-center">
            <label className="flex items-center gap-2 text-xs text-text-secondary cursor-pointer">
              <input
                type="checkbox"
                checked={useCoulombFriction}
                onChange={(e) => setUseCoulombFriction(e.target.checked)}
                className="accent-accent"
              />
              Coulomb friction model
              <span className="text-text-muted">(per-contact-point, more accurate)</span>
            </label>
            <label className="flex items-center gap-2 text-xs text-text-secondary cursor-pointer">
              <input
                type="checkbox"
                checked={frictionCombineMax}
                onChange={(e) => setFrictionCombineMax(e.target.checked)}
                className="accent-accent"
              />
              Max friction combine rule
              <span className="text-text-muted">(higher friction always wins)</span>
            </label>
          </div>
        </div>
      )}

      <div className="flex gap-3 mt-6">
        <button
          onClick={handleSubmit}
          disabled={creating}
          className="px-5 py-2 bg-accent text-white rounded-md text-sm font-medium hover:bg-accent-hover transition-colors disabled:opacity-50"
        >
          {creating ? "Starting..." : "Start Evolution"}
        </button>
        <button
          onClick={onCancel}
          className="px-5 py-2 bg-bg-elevated text-text-secondary rounded-md text-sm hover:text-text-primary transition-colors"
        >
          Cancel
        </button>
      </div>
    </div>
  );
}
