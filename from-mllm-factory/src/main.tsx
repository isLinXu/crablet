import { createApp } from "vue";
import App from "./App.vue";
import { invoke } from "@tauri-apps/api/core";

createApp(App).mount("#app");
