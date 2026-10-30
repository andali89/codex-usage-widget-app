import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { startDiagnostics } from "./useDiagnostics";
import App from "./App";
import SurfaceView from "./SurfaceView";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "@tauri-apps/api/core";
import "./styles.css";

startDiagnostics();
const root = document.getElementById("root");

if (!root) {
  throw new Error("Application root element is missing.");
}

const surface = isTauri() ? getCurrentWindow().label : "main";
if (surface === "shell" || surface === "preview") {
  document.documentElement.dataset.surface = surface;
  document.body.dataset.surface = surface;
}
createRoot(root).render(
  <StrictMode>
    {surface === "shell" || surface === "preview" ? <SurfaceView surface={surface} /> : <App />}
  </StrictMode>,
);
