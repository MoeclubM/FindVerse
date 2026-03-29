import React from "react";
import ReactDOM from "react-dom/client";

import "./i18n";
import { App } from "./app";
import "./index.css";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
