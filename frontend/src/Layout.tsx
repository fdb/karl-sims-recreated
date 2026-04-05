import type { ReactNode } from "react";
import { Link, useLocation } from "@tanstack/react-router";

interface Props {
  children: ReactNode;
}

export default function Layout({ children }: Props) {
  const location = useLocation();
  const path = location.pathname;
  const isDashboardActive =
    path === "/" || path.startsWith("/evolutions/");
  const isViewerActive = path === "/viewer";

  return (
    <div className="min-h-screen bg-bg-base text-text-primary">
      <header className="bg-bg-header border-b border-border sticky top-0 z-50">
        <div className="max-w-[1600px] mx-auto px-6 h-14 flex items-center justify-between">
          <Link
            to="/"
            className="text-base sm:text-lg font-semibold text-text-primary hover:text-accent transition-colors truncate no-underline"
          >
            <span className="hidden sm:inline">
              Evolving Virtual Creatures
            </span>
            <span className="sm:hidden">Virtual Creatures</span>
          </Link>
          <nav className="flex gap-1">
            <Link
              to="/"
              className={`px-4 py-2 rounded-md text-sm transition-colors no-underline ${
                isDashboardActive
                  ? "bg-bg-elevated text-text-primary"
                  : "text-text-secondary hover:text-text-primary hover:bg-bg-surface"
              }`}
            >
              Dashboard
            </Link>
            <Link
              to="/viewer"
              className={`px-4 py-2 rounded-md text-sm transition-colors no-underline ${
                isViewerActive
                  ? "bg-bg-elevated text-text-primary"
                  : "text-text-secondary hover:text-text-primary hover:bg-bg-surface"
              }`}
            >
              Viewer
            </Link>
          </nav>
        </div>
      </header>

      <main className="max-w-[1600px] mx-auto px-6 py-6">{children}</main>
    </div>
  );
}
