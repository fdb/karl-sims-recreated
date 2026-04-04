import { useState, useEffect } from "react";
import { parseRoute, type Route } from "../router";

export function useRoute(): Route {
  const [route, setRoute] = useState<Route>(parseRoute());

  useEffect(() => {
    const handler = () => setRoute(parseRoute());
    window.addEventListener("popstate", handler);
    return () => window.removeEventListener("popstate", handler);
  }, []);

  return route;
}
