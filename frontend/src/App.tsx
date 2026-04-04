import { useRoute } from "./hooks/useRoute";
import { navigate } from "./router";
import EvolutionList from "./pages/EvolutionList";
import EvolutionDetail from "./pages/EvolutionDetail";
import CreatureDetail from "./pages/CreatureDetail";
import Viewer from "./pages/Viewer";
import "./App.css";

export default function App() {
  const route = useRoute();

  return (
    <div className="min-h-screen bg-bg-base text-text-primary">
      {/* Header */}
      <header className="bg-bg-header border-b border-border sticky top-0 z-50">
        <div className="max-w-[1600px] mx-auto px-6 h-14 flex items-center justify-between">
          <a
            href="/"
            onClick={(e) => {
              e.preventDefault();
              navigate("/");
            }}
            className="text-lg font-semibold text-text-primary hover:text-accent transition-colors"
          >
            Evolving Virtual Creatures
          </a>
          <nav className="flex gap-1">
            <a
              href="/"
              onClick={(e) => {
                e.preventDefault();
                navigate("/");
              }}
              className={`px-4 py-2 rounded-md text-sm transition-colors ${
                route.path === "home" ||
                route.path === "evolution" ||
                route.path === "creature"
                  ? "bg-bg-elevated text-text-primary"
                  : "text-text-secondary hover:text-text-primary hover:bg-bg-surface"
              }`}
            >
              Dashboard
            </a>
            <a
              href="/viewer"
              onClick={(e) => {
                e.preventDefault();
                navigate("/viewer");
              }}
              className={`px-4 py-2 rounded-md text-sm transition-colors ${
                route.path === "viewer"
                  ? "bg-bg-elevated text-text-primary"
                  : "text-text-secondary hover:text-text-primary hover:bg-bg-surface"
              }`}
            >
              Viewer
            </a>
          </nav>
        </div>
      </header>

      {/* Main content */}
      <main className="max-w-[1600px] mx-auto px-6 py-6">
        {route.path === "home" && <EvolutionList />}
        {route.path === "evolution" && (
          <EvolutionDetail evoId={Number(route.params.evoId)} />
        )}
        {route.path === "creature" && (
          <CreatureDetail
            evoId={Number(route.params.evoId)}
            creatureId={Number(route.params.creatureId)}
          />
        )}
        {route.path === "viewer" && <Viewer />}
      </main>
    </div>
  );
}
