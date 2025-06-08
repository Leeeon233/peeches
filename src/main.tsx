import React from "react";
import ReactDOM from "react-dom/client";
import { HashRouter } from "react-router-dom";
import App from "./App";
import { JotaiProvider } from "./providers/JotaiProvider";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <JotaiProvider>
      <HashRouter>
        <App />
      </HashRouter>
    </JotaiProvider>
  </React.StrictMode>
);
