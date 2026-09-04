import "./style.css";
import { mount } from "svelte";
import App from "./App.svelte";

const target = document.getElementById("app");
if (target === null) throw new Error("le point de montage est absent");

mount(App, { target });
