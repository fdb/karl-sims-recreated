export interface Route {
  path: string;
  params: Record<string, string>;
}

export function parseRoute(): Route {
  const path = window.location.pathname;

  // /evolutions/:id/creatures/:creatureId
  let match = path.match(/^\/evolutions\/(\d+)\/creatures\/(\d+)$/);
  if (match)
    return {
      path: "creature",
      params: { evoId: match[1], creatureId: match[2] },
    };

  // /evolutions/:id
  match = path.match(/^\/evolutions\/(\d+)$/);
  if (match) return { path: "evolution", params: { evoId: match[1] } };

  // /viewer
  if (path === "/viewer") return { path: "viewer", params: {} };

  // / (home)
  return { path: "home", params: {} };
}

export function navigate(url: string) {
  history.pushState(null, "", url);
  window.dispatchEvent(new PopStateEvent("popstate"));
}
