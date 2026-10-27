<script setup lang="ts">
// Le logo de `assets/logo/` (version sans fond).
import url from "../../../assets/logo/logo.svg?url";

defineProps<{ size?: number }>();
</script>

<template>
  <img :src="url" :width="size ?? 28" :height="size ?? 28" alt="" aria-hidden="true" />
</template>

<style scoped>
img { display: block; flex: none; }
</style>
