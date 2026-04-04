import { useEffect, useState } from "react";
import type { GenerationStats } from "../api";

export function useEvolutionUpdates(): GenerationStats[] {
  const [stats, setStats] = useState<GenerationStats[]>([]);

  useEffect(() => {
    let ws: WebSocket | null = null;
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
    let closed = false;

    function connect() {
      if (closed) return;
      try {
        ws = new WebSocket("ws://localhost:3000/api/live");

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
          } catch {
            // ignore parse errors
          }
        };

        ws.onclose = () => {
          if (!closed) {
            // Reconnect after 3 seconds
            reconnectTimer = setTimeout(connect, 3000);
          }
        };

        ws.onerror = () => {
          ws?.close();
        };
      } catch {
        // Connection failed, retry
        if (!closed) {
          reconnectTimer = setTimeout(connect, 3000);
        }
      }
    }

    connect();

    return () => {
      closed = true;
      if (reconnectTimer) clearTimeout(reconnectTimer);
      ws?.close();
    };
  }, []);

  return stats;
}
