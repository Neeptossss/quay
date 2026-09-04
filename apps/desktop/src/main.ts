import "./style.css";
import { mount } from "svelte";
import { invoke } from "@tauri-apps/api/core";
import App from "./App.svelte";

const target = document.getElementById("app");
if (target === null) throw new Error("le point de montage est absent");

void invoke("bundle_loaded").catch(() => {});
mount(App, { target });
