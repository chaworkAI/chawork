import React from "react"
import ReactDOM from "react-dom/client"
import { initLocaleStore } from "@/stores/locale"
import { App } from "./App"
import "./styles.css"

initLocaleStore()

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
)
