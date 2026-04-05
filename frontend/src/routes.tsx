import {
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
} from "@tanstack/react-router";
import Layout from "./Layout";
import EvolutionList from "./pages/EvolutionList";
import EvolutionDetail from "./pages/EvolutionDetail";
import CreatureDetail from "./pages/CreatureDetail";
import Viewer from "./pages/Viewer";

/**
 * Code-based routes. Each route gets typed params thanks to the path
 * template — `useParams({ from: creatureRoute.id })` returns
 * `{ evoId: string; creatureId: string }` with no casts.
 *
 * For islands, we have two parallel creature routes that both resolve
 * to CreatureDetail — one with the /islands/:islandId segment and one
 * without. This lets us keep island context in the URL when drilling
 * into a creature from the island UI, without forcing every creature
 * link to synthesize an island.
 */

const rootRoute = createRootRoute({
  component: () => (
    <Layout>
      <Outlet />
    </Layout>
  ),
});

const homeRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: EvolutionList,
});

const viewerRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/viewer",
  component: Viewer,
});

const evolutionRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/evolutions/$evoId",
  component: function EvolutionRouteView() {
    const { evoId } = evolutionRoute.useParams();
    return <EvolutionDetail evoId={Number(evoId)} />;
  },
});

const creatureRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/evolutions/$evoId/creatures/$creatureId",
  component: function CreatureRouteView() {
    const { evoId, creatureId } = creatureRoute.useParams();
    return (
      <CreatureDetail evoId={Number(evoId)} creatureId={Number(creatureId)} />
    );
  },
});

const islandCreatureRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/evolutions/$evoId/islands/$islandId/creatures/$creatureId",
  component: function IslandCreatureRouteView() {
    const { evoId, islandId, creatureId } = islandCreatureRoute.useParams();
    return (
      <CreatureDetail
        evoId={Number(evoId)}
        creatureId={Number(creatureId)}
        islandId={Number(islandId)}
      />
    );
  },
});

const routeTree = rootRoute.addChildren([
  homeRoute,
  viewerRoute,
  evolutionRoute,
  creatureRoute,
  islandCreatureRoute,
]);

export const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
