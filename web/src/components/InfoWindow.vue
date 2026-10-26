<script setup lang="ts">
// La fenêtre « i » : un texte Markdown simple (gras, italique, liens, listes) fourni par celui qui compile le site.
// Le HTML brut est refusé et les adresses douteuses (`javascript:`) sont écartées par la bibliothèque.
import MarkdownIt from "markdown-it";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import FloatingWindow from "./FloatingWindow.vue";

const emit = defineEmits(["close"]);
const { t } = useI18n();

const markdown = new MarkdownIt({ html: false, linkify: true, breaks: false }).disable(["table", "image", "code", "fence", "html_block", "html_inline"]);
const links = markdown.renderer.rules.link_open ?? ((tokens, i, options, _env, self) => self.renderToken(tokens, i, options));
markdown.renderer.rules.link_open = (tokens, i, options, env, self) => {
  tokens[i]?.attrSet("target", "_blank");
  tokens[i]?.attrSet("rel", "noopener noreferrer");
  return links(tokens, i, options, env, self);
};

const html = computed(() => markdown.render(__NWASABI_INFO__));
const title = __NWASABI_INFO_TITLE__ || t("info.title");
</script>

<template>
  <FloatingWindow :title="title" :width="440" @close="emit('close')">
    <div class="md" v-html="html" />
  </FloatingWindow>
</template>

<style scoped>
.md :deep(p) { margin: 0 0 8px; }
.md :deep(p:last-child), .md :deep(ul:last-child) { margin-bottom: 0; }
.md :deep(h1), .md :deep(h2), .md :deep(h3) { font-size: 14px; margin: 10px 0 4px; }
.md :deep(ul), .md :deep(ol) { margin: 0 0 8px; padding-left: 20px; }
.md :deep(a) { color: var(--accent-strong); }
</style>
