import { createApp } from "vue";
import App from "./App.vue";
import { i18n } from "./i18n.js";
import "./style.css";
import logo from "../../assets/logo/logo-bordered.svg?url";

createApp(App).use(i18n).mount("#app");

// Icône de l'onglet : lue dans assets/logo, comme les autres images. La page statique ne peut pas la nommer, car son adresse
// change selon le mode (développement ou site construit, où elle porte une empreinte).
document.head.append(Object.assign(document.createElement("link"), { rel: "icon", type: "image/svg+xml", href: logo }));
