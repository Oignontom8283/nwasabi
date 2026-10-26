<script setup lang="ts">
// Licence et conditions d'utilisation, dans une fenêtre flottante.
import { useI18n } from "vue-i18n";
import FloatingWindow from "./FloatingWindow.vue";
import Icon from "./Icon.vue";

const emit = defineEmits(["close"]);
const { t } = useI18n();
const REPO = "https://github.com/Oignontom8283/nwasabi";
const SECTIONS = ["license", "terms", "third", "trademarks"];
</script>

<template>
  <FloatingWindow :title="t('about.title')" :width="480" @close="emit('close')">
    <section v-for="id in SECTIONS" :key="id" class="section">
      <h2>{{ t(`about.${id}.title`) }}</h2>
      <p>{{ t(`about.${id}.text`) }}</p>
    </section>
    <p class="links"><a :href="REPO" target="_blank" rel="noopener noreferrer"><Icon name="github" :size="14" /> {{ t("about.repo") }}</a></p>
  </FloatingWindow>
</template>

<style scoped>
.section h2 { font-size: 13px; margin: 0 0 2px; }
.section p { margin: 0 0 10px; }
.links { margin: 4px 0 0; }
a { color: var(--accent-strong); display: inline-flex; gap: 6px; align-items: center; }
</style>
