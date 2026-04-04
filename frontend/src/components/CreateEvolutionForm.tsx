import { useState } from "react";
import { createEvolution } from "../api";

interface Props {
  onCreated: () => void;
  onCancel: () => void;
}

export default function CreateEvolutionForm({ onCreated, onCancel }: Props) {
  const [goal, setGoal] = useState("swimming_speed");
  const [environment, setEnvironment] = useState("water");
  const [popSize, setPopSize] = useState(50);
  const [maxGen, setMaxGen] = useState(100);
  const [simDuration, setSimDuration] = useState(10);
  const [maxParts, setMaxParts] = useState(20);
  const [gravity, setGravity] = useState(9.81);
  const [viscosity, setViscosity] = useState(2.0);
  const [name, setName] = useState("");
  const [creating, setCreating] = useState(false);

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
            placeholder="e.g. Water swimmers v2"
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
              : "Gravity at 9.81 m/s², ground plane with collisions."}
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
            <label className={labelClass}>Gravity (m/s²)</label>
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
      </div>

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
