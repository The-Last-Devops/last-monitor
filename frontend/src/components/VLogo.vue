<script setup>
// The Vantage brand mark — one place so the logo can change everywhere at once.
// Registered globally in main.js: <VLogo :size="32" />. The `.vantage-logo` class
// (style.css) supplies the teal tile + the gentle hue animation driven by App.vue.
import { computed } from 'vue'
const props = defineProps({ size: { type: [Number, String], default: 24 } })
const px = computed(() => `${props.size}px`)
const inner = computed(() => Math.round(Number(props.size) * 0.62))
</script>

<template>
  <span class="vantage-logo grid shrink-0 place-items-center rounded-lg" :style="{ width: px, height: px }">
    <svg viewBox="0 0 64 64" :width="inner" :height="inner" fill="none" stroke="#08231F"
      stroke-width="5.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M7 43 L21 21 L35 43" />
      <rect x="42" y="37.5" width="14" height="6" rx="1.8" fill="#08231F" stroke="none" />
    </svg>
  </span>
</template>
