import { createApp } from "vue";
import App from "./App.vue";
import Popup from "./Popup.vue";
import { applyTheme, watchTheme } from "./theme.js";
import "./theme.css";

applyTheme();
watchTheme();

const popup = new URLSearchParams(window.location.search).get("popup") === "1";
createApp(popup ? Popup : App).mount("#app");
