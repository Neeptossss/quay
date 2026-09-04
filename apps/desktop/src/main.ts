import "./design/tokens.css";
import "./design/base.css";
import "./design/app.css";
import { mount } from "svelte";
import { invoke } from "@tauri-apps/api/core";
import App from "./App.svelte";
import { load } from "./i18n";

const target = document.getElementById("app");
if (target === null) throw new Error("le point de montage est absent");

void invoke("bundle_loaded").catch(() => {});
load()
  .catch(() => {})
  .finally(() => {
    mount(App, { target });
    void invoke("mark", { phase: "mount returned" }).catch(() => {});
  });
