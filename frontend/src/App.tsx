import { RouterProvider } from "@tanstack/react-router";
import { router } from "./routes";
import "./App.css";

export default function App() {
  return <RouterProvider router={router} />;
}
