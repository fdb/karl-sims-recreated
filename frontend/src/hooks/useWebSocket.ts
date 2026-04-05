import { useEffect, useState } from "react";
import type { GenerationStats } from "../api";

export function useEvolutionUpdates(): GenerationStats[] {
  const [stats, setStats] = useState<GenerationStats[]>([]);

  useEffect(() => {
    let ws: WebSocket | null = null;
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
    // Deferred-open timer. React StrictMode double-mounts every effect in
    // dev: mount → cleanup → mount. If we open a WebSocket synchronously on
    // mount, the cleanup closes it *before* the 101 Upgrade handshake
    // completes, giving us the noisy "closed before connection established"
    // browser warning (plus an EPIPE from the Vite dev-proxy). By deferring
    // `new WebSocket` to the next macrotask, StrictMode's cleanup runs first
    // and cancels this timer — we never open that doomed socket.
    let openTimer: ReturnType<typeof setTimeout> | null = null;
    let closed = false;

    function connect() {
      if (closed) return;
      openTimer = setTimeout(() => {
        openTimer = null;
        if (closed) return;
        openSocket();
      }, 0);
    }

    function openSocket() {
      try {
        const wsProto = window.location.protocol === "https:" ? "wss:" : "ws:";
        ws = new WebSocket(`${wsProto}//${window.location.host}/api/live`);

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
      if (openTimer) clearTimeout(openTimer);
      if (reconnectTimer) clearTimeout(reconnectTimer);
      ws?.close();
    };
  }, []);

  return stats;
}
