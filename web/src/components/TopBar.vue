<script setup lang="ts">
import { useEmu } from "../emulator.js";
import { computed, inject, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { LOCALES, setLocale } from "../i18n.js";
import { reload, server } from "../serverLink.js";
import { isCompact, recording, screenApi, ui, videoRecording } from "../ui.js";
import type { PickKind, Theme } from "../ui.js";
import { valueOf } from "../util.js";
import type { Firmware, Model } from "@nwasabi/js";
import { recent } from "../recent.js";
import { keyed } from "../shortcuts.js";
import Icon from "./Icon.vue";
import Logo from "./Logo.vue";

const emit = defineEmits<{ pick: [which: PickKind]; recent: [name: string]; about: []; info: [] }>();
const hasInfo = __NWASABI_INFO__ !== "";
const { t, locale } = useI18n();
const emulator = useEmu();
const { state } = emulator;

const REPO = "https://github.com/Oignontom8283/nwasabi";
const THEMES: Theme[] = ["auto", "light", "dark"];
const THEME_ICON: Record<Theme, string> = { auto: "monitor", light: "sun", dark: "moon" };
const themeLabel = computed(() => t("top.theme", { theme: t(`top.themes.${ui.theme}`) }));
const nextTheme = () => (ui.theme = THEMES[(THEMES.indexOf(ui.theme) + 1) % THEMES.length] ?? "auto");

// Le menu des imports : trois traits à gauche ; il se ferme en cliquant ailleurs ou avec Échap.
const open = ref(false);
// Sur un écran compact, les réglages de la barre sont rangés dans un second menu (les trois points à droite).
const compact = computed(() => isCompact());
const more = ref(false);
const moreRoot = ref<HTMLElement | null>(null);
// Le logo ramène à l'accueil du site : la documentation quand l'émulateur est servi dans le site (`…/emulator/`), sinon (serveur
// local) l'émulateur lui-même. Sans paramètres ni ancre.
const home = new URL(location.pathname.includes("/emulator/") ? "../" : "./", location.href).pathname;
const root = ref<HTMLElement | null>(null);
const ITEMS: Array<[PickKind, string, boolean]> = [
  ["app", "top.openApp", true],
  ["data", "top.openData", false],
  ["keys", "top.openKeys", false],
  ["files", "top.openFiles", false],
  ["flash", "top.openFlash", false],
  ["state", "top.openState", false],
];
function openRecent(name: string) {
  open.value = false;
  emit("recent", name);
}
function pick(which: PickKind) {
  open.value = false;
  emit("pick", which);
}
const outside = (e: Event) => {
  if (open.value && !root.value?.contains(e.target as Node)) open.value = false;
  if (more.value && !moreRoot.value?.contains(e.target as Node)) more.value = false;
};
const escape = (e: KeyboardEvent) => {
  if (e.key !== "Escape" || !(open.value || more.value)) return;
  open.value = more.value = false;
  e.stopPropagation();
};
/** Un seul des deux menus est ouvert à la fois. */
const toggleMenu = () => ((open.value = !open.value), (more.value = false));
const toggleMore = () => ((more.value = !more.value), (open.value = false));
const capture = () => screenApi.capture();
const toggleGif = () => screenApi.toggleRecording();
const toggleVideo = () => screenApi.toggleVideo();
const fullscreen = () => screenApi.toggleFullscreen();
const toggleLarge = () => (ui.large = !ui.large);
/** Une action du menu des réglages : on le referme ensuite. */
const act = (run: () => void) => () => {
  more.value = false;
  run();
};
onMounted(() => {
  addEventListener("pointerdown", outside);
  addEventListener("keydown", escape, true);
});
onBeforeUnmount(() => {
  removeEventListener("pointerdown", outside);
  removeEventListener("keydown", escape, true);
});
</script>

<template>
  <header class="topbar">
    <div ref="root" class="menu-wrap">
      <button class="burger" :aria-expanded="open" aria-haspopup="menu" :aria-label="t('top.menu')" :title="t('top.menu')" @click="toggleMenu">
        <Icon name="burger" :size="22" />
      </button>
      <ul v-if="open" class="menu" role="menu">
        <li v-for="[id, key, primary] in ITEMS" :key="id" role="none">
          <button role="menuitem" :class="{ primary }" :title="t(`${key}Hint`)" @click="pick(id)">{{ t(key) }}</button>
        </li>
        <template v-if="recent.items.length">
          <li class="sub" role="presentation">{{ t("top.recent") }}</li>
          <li v-for="item in recent.items" :key="item.name" role="none">
            <button role="menuitem" class="mono" :title="t('top.recentHint', { when: new Date(item.at).toLocaleString(locale) })" @click="openRecent(item.name)">{{ item.name }}</button>
          </li>
        </template>
      </ul>
    </div>

    <a class="brand" :href="home" :title="t('top.about')"><Logo :size="34" /> <span class="brand-text">Nwasabi</span></a>

    <template v-if="!compact">
      <select :value="state.model" :title="t('top.modelHint')" :aria-label="t('top.model')" @change="(e: Event) => emulator.changeModel(valueOf(e) as Model)">
        <option value="n0110">N0110</option>
        <option value="n0115">N0115</option>
        <option value="n0120">N0120</option>
      </select>
      <select :value="state.firmware" :title="t('top.firmwareHint')" :aria-label="t('top.firmware')" @change="(e: Event) => emulator.changeModel(state.model, valueOf(e) as Firmware)">
        <option value="24">Epsilon 23–24</option>
        <option value="26.4.0">Epsilon 26.4.0</option>
      </select>
      <button :disabled="!state.loaded" :title="keyed(state.running ? t('top.pauseHint') : t('top.resumeHint'), 'toggleRun')" @click="state.running ? emulator.stop() : emulator.start()">
        <Icon :name="state.running ? 'pause' : 'play'" :size="14" /> {{ state.running ? t("top.pause") : t("top.resume") }}
      </button>
      <button :disabled="!state.loaded || state.running" :title="keyed(t('top.stepHint'), 'frame')" @click="emulator.stepFrame()"><Icon name="step" :size="14" /> {{ t("top.step") }}</button>
      <button :disabled="!state.loaded" :title="keyed(t('top.restartHint'), 'restart')" @click="emulator.restart()"><Icon name="refresh" :size="14" /> {{ t("top.restart") }}</button>
      <button v-if="server.stale.length" class="reload" :title="keyed(t('top.reloadHint'), 'reload')" @click="reload()"><Icon name="refresh" :size="14" /> {{ t("top.reload") }}</button>
      <button v-if="hasInfo" class="icon" :title="t('top.info')" :aria-label="t('top.info')" @click="emit('info')"><Icon name="info" :size="16" /></button>

      <div class="center" :class="{ idle: !state.appName }">
        <span class="name" :class="{ none: !state.appName }" :title="state.appName">{{ state.appName || t("top.noApp") }}</span>
        <button v-if="!state.loaded && !state.needsData" class="load primary" :title="t('top.openAppHint')" @click="pick('app')"><Icon name="upload" :size="14" /> {{ t("top.load") }}</button>
      </div>

      <div class="right">
        <select :value="locale" :aria-label="t('top.language')" :title="t('top.language')" @change="(e: Event) => setLocale(valueOf(e))">
          <option v-for="[code, name] in LOCALES" :key="code" :value="code">{{ name }}</option>
        </select>
        <button class="flat icon" :class="{ on: ui.large }" :title="keyed(t('top.large'), 'large')" :aria-label="t('top.large')" :aria-pressed="ui.large" @click="ui.large = !ui.large"><Icon name="expand" :size="18" /></button>
        <button class="flat icon" :title="themeLabel" :aria-label="themeLabel" @click="nextTheme"><Icon :name="THEME_ICON[ui.theme]" :size="18" /></button>
        <button class="flat icon" :title="t('top.aboutWindow')" :aria-label="t('top.aboutWindow')" @click="emit('about')"><Icon name="license" :size="18" /></button>
        <a class="flat icon link" :href="REPO" target="_blank" rel="noopener noreferrer" :title="t('top.repo')" :aria-label="t('top.repo')"><Icon name="github" :size="18" /></a>
      </div>
    </template>

    <template v-else>
      <span class="name compact-name" :class="{ none: !state.appName }" :title="state.appName">{{ state.appName || t("top.noApp") }}</span>
      <button v-if="!state.loaded && !state.needsData" class="compact-load primary" :title="t('top.openAppHint')" @click="pick('app')"><Icon name="upload" :size="14" /> {{ t("top.load") }}</button>
      <button :disabled="!state.loaded" class="icon run" :title="state.running ? t('top.pauseHint') : t('top.resumeHint')" :aria-label="state.running ? t('top.pause') : t('top.resume')" @click="state.running ? emulator.stop() : emulator.start()">
        <Icon :name="state.running ? 'pause' : 'play'" :size="18" />
      </button>
      <div ref="moreRoot" class="menu-wrap">
        <button class="icon dots" :aria-expanded="more" aria-haspopup="menu" :aria-label="t('top.more')" :title="t('top.more')" @click="toggleMore"><Icon name="dots" :size="20" /></button>
        <div v-if="more" class="menu more-menu" role="menu">
          <label class="field">{{ t("top.model") }}
            <select :value="state.model" :title="t('top.modelHint')" @change="(e: Event) => emulator.changeModel(valueOf(e) as Model)">
              <option value="n0110">N0110</option>
              <option value="n0115">N0115</option>
              <option value="n0120">N0120</option>
            </select>
          </label>
          <label class="field">{{ t("top.firmware") }}
            <select :value="state.firmware" :title="t('top.firmwareHint')" @change="(e: Event) => emulator.changeModel(state.model, valueOf(e) as Firmware)">
              <option value="24">Epsilon 23–24</option>
              <option value="26.4.0">Epsilon 26.4.0</option>
            </select>
          </label>
          <button role="menuitem" :disabled="!state.loaded || state.running" @click="act(() => emulator.stepFrame())()"><Icon name="step" :size="16" /> {{ t("top.step") }}</button>
          <button role="menuitem" :disabled="!state.loaded" @click="act(() => emulator.restart())()"><Icon name="refresh" :size="16" /> {{ t("top.restart") }}</button>
          <button v-if="server.stale.length" role="menuitem" class="reload" @click="act(reload)()"><Icon name="refresh" :size="16" /> {{ t("top.reload") }}</button>
          <button role="menuitem" :disabled="!state.loaded" @click="act(capture)()"><Icon name="camera" :size="16" /> {{ t("mini.capture") }}</button>
          <button role="menuitem" :disabled="!state.loaded" :aria-pressed="recording.on" @click="act(toggleGif)()"><Icon name="dot" :size="16" /> {{ recording.on ? t("mini.recordStop") : t("mini.record") }}</button>
          <button role="menuitem" :disabled="!state.loaded" :aria-pressed="videoRecording.on" @click="act(toggleVideo)()"><Icon name="video" :size="16" /> {{ videoRecording.on ? t("mini.videoStop") : t("mini.video") }}</button>
          <button role="menuitem" :disabled="!state.loaded" @click="act(fullscreen)()"><Icon name="maximize" :size="16" /> {{ t("mini.fullscreen") }}</button>
          <button role="menuitem" :aria-pressed="ui.large" @click="act(toggleLarge)()"><Icon name="expand" :size="16" /> {{ t("mini.large") }}</button>
          <button role="menuitem" @click="nextTheme"><Icon :name="THEME_ICON[ui.theme]" :size="16" /> {{ themeLabel }}</button>
          <label class="field">{{ t("top.language") }}
            <select :value="locale" @change="(e: Event) => setLocale(valueOf(e))">
              <option v-for="[code, name] in LOCALES" :key="code" :value="code">{{ name }}</option>
            </select>
          </label>
          <button v-if="hasInfo" role="menuitem" @click="act(() => emit('info'))()"><Icon name="info" :size="16" /> {{ t("top.info") }}</button>
          <button role="menuitem" @click="act(() => emit('about'))()"><Icon name="license" :size="16" /> {{ t("top.aboutWindow") }}</button>
          <a role="menuitem" class="item" :href="REPO" target="_blank" rel="noopener noreferrer"><Icon name="github" :size="16" /> {{ t("top.repo") }}</a>
        </div>
      </div>
    </template>
  </header>
</template>

<style scoped>
.topbar { height: var(--topbar-h); display: flex; align-items: center; gap: 8px; padding: 0 12px; background: var(--surface); border-bottom: 1px solid var(--line); position: relative; z-index: 20; }
.menu-wrap { position: relative; }
.burger { border: none; background: transparent; padding: 4px; border-radius: 6px; color: var(--text); }
.menu { position: absolute; top: calc(100% + 6px); left: 0; list-style: none; margin: 0; padding: 6px; min-width: 260px; display: grid; gap: 3px; background: var(--surface); border: 1px solid var(--line); border-radius: 8px; box-shadow: var(--shadow); }
.menu .sub { margin: 6px 10px 0; font-size: 11px; letter-spacing: 0.08em; text-transform: uppercase; color: var(--muted); }
.menu button { width: 100%; justify-content: flex-start; padding: 6px 10px; }
.brand { display: inline-flex; align-items: center; gap: 8px; font-weight: 700; font-size: 20px; letter-spacing: 0.01em; color: var(--text); text-decoration: none; margin-right: 18px; }
/* Le nom reste exactement au centre de la barre ; le bouton de chargement se pose à sa droite sans le décaler. */
.center { position: absolute; left: 50%; transform: translateX(-50%); max-width: 28%; }
.name { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--muted); pointer-events: none; }
.load { position: absolute; left: 100%; top: 50%; transform: translateY(-50%); margin-left: 12px; }
.right { margin-left: auto; display: flex; align-items: center; gap: 4px; }
.right select { margin-right: 4px; }
a.link { display: inline-flex; align-items: center; border-radius: 6px; padding: 3px 6px; color: var(--text); border: 1px solid transparent; }
a.link:hover { background-image: linear-gradient(var(--hover), var(--hover)); }
/* Sans application, le texte « Aucune application » cède la place au seul bouton quand la barre est étroite. */
@media (max-width: 1560px) { .center:not(.idle) { position: static; transform: none; margin-left: 12px; max-width: 220px; } .center.idle { transform: none; } .center.idle .name { display: none; } .center.idle .load { position: static; transform: none; margin: 0 0 0 12px; } }
@media (max-width: 1330px) { .center { display: none; } }
@media (max-width: 700px) { .brand span { display: none; } }
/* Écran compact : le nom de l'application occupe la place qui reste, les réglages sont dans le menu des trois points. */
.compact-name { flex: 1; min-width: 0; text-align: left; }
.compact-load { flex: none; }
.run { flex: none; }
.dots { border: none; background: transparent; }
.more-menu { left: auto; right: 0; min-width: 250px; max-width: calc(100vw - 24px); max-height: calc(100dvh - var(--topbar-h) - 16px); overflow-y: auto; }
.more-menu label.field { grid-template-columns: 1fr auto; align-items: center; gap: 8px; padding: 2px 6px; font-size: 13px; }
.more-menu button, .more-menu .item { width: 100%; justify-content: flex-start; padding: 8px 10px; }
.more-menu .item { display: flex; align-items: center; gap: 5px; border-radius: 6px; border: 1px solid var(--line); background: var(--key); color: var(--text); text-decoration: none; }
@media (max-width: 999px) { .brand { margin-right: 4px; } }
@media (max-width: 479px) { .brand-text { display: none; } }
</style>
