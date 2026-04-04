import type { GenotypeInfo } from "../api";

export default function BrainGraph({ info }: { info: GenotypeInfo }) {
  // Collect all neurons across all body parts
  const allNeurons: { partId: number; neuronId: number; func: string; inputs: { source: string; weight: number }[] }[] = [];

  for (const node of info.nodes) {
    for (const neuron of node.brain.neurons) {
      allNeurons.push({ partId: node.id, neuronId: neuron.id, func: neuron.func, inputs: neuron.inputs });
    }
  }

  if (allNeurons.length === 0) {
    return <p className="empty">No neurons in this creature.</p>;
  }

  const rowH = 32, pad = 20;

  // Group neurons by part
  const byPart = new Map<number, typeof allNeurons>();
  for (const n of allNeurons) {
    if (!byPart.has(n.partId)) byPart.set(n.partId, []);
    byPart.get(n.partId)!.push(n);
  }

  let y = pad;
  const elements: JSX.Element[] = [];

  for (const [partId, neurons] of byPart) {
    // Part header
    elements.push(
      <text key={`ph${partId}`} x={pad} y={y + 14} fill="#4caf50" fontSize="11" fontWeight="600">
        Part {partId}
      </text>
    );
    y += 22;

    for (const neuron of neurons) {
      // Neuron box
      const funcColors: Record<string, string> = {
        Sum: "#4a6fa5", Product: "#7b5ea7", Sigmoid: "#a5694f",
        Sin: "#5a8f6a", OscillateWave: "#8f8f3a", Memory: "#7a4a5a",
      };
      const color = funcColors[neuron.func] || "#555";

      elements.push(
        <g key={`n${partId}-${neuron.neuronId}`}>
          <rect x={pad} y={y} width={100} height={24} rx="4" fill={color} opacity="0.8" />
          <text x={pad + 50} y={y + 16} fill="#e0e0e0" fontSize="10" textAnchor="middle">
            {neuron.func}
          </text>
          {neuron.inputs.map((inp, i) => (
            <text key={i} x={130 + i * 150} y={y + 16} fill="#999" fontSize="9">
              {"\u2190"} {inp.source.replace(/"/g, "")} (w:{inp.weight.toFixed(2)})
            </text>
          ))}
        </g>
      );
      y += rowH;
    }
    y += 8; // gap between parts
  }

  const svgW = 600;

  return <svg width={svgW} height={y + pad} className="brain-graph">{elements}</svg>;
}
