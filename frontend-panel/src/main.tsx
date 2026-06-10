import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
import "@/i18n";
import App from "./App.tsx";

const root = document.getElementById("root");
if (!root) throw new Error("Root element not found");
root.querySelector("[data-aster-boot-loading]")?.remove();
createRoot(root).render(
	<StrictMode>
		<App />
	</StrictMode>,
);
