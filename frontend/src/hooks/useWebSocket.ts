import { useEffect, useRef, useState } from "react";
import type { GenerationStats } from "../api";

export function useEvolutionUpdates(): GenerationStats[] {
  const [stats, setStats] = useState<GenerationStats[]>([]);
  const wsRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    const ws = new WebSocket(`ws://localhost:3000/api/live`);
    wsRef.current = ws;

    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        if (data.type === "generation") {
          setStats((prev) => [
            ...prev,
            {
              evolution_id: data.evolution_id,
              generation: data.generation,
              best_fitness: data.best_fitness,
              avg_fitness: data.avg_fitness,
            },
          ]);
        }
      } catch (e) {
        console.warn("WS parse error:", e);
      }
    };

    ws.onerror = () => console.warn("WebSocket error");
    ws.onclose = () => console.log("WebSocket closed");

    return () => {
      ws.close();
    };
  }, []);

  return stats;
}
