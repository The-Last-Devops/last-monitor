<script setup>
// A status pill (Rancher-style): colored dot + label, tinted background + border.
// Colors flip with the theme (see .vantage-pill-* in style.css).
const props = defineProps({
  tone: { type: String, default: 'muted' }, // ok | warn | down | info | muted
  label: { type: String, required: true },
  dot: { type: Boolean, default: true },
})
</script>

<template>
  <span class="vantage-pill" :class="`vantage-pill-${props.tone}`"><span v-if="dot" class="d"></span>{{ label }}</span>
</template>
