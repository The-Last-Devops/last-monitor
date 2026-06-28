<script setup>
// Themed icon — renders a named glyph from the design-system icon library on the
// 24px grid (2px round stroke, currentColor). Sizes per handoff: 16 rows / 20 nav /
// 24 headers. Inherits colour from `currentColor`, so wrap in a text-* class.
import { computed } from 'vue'
import { ICONS } from '../lib/icons'

const props = defineProps({
  name: { type: String, required: true },
  size: { type: [Number, String], default: 20 },
})
const inner = computed(() => ICONS[props.name] || '')
</script>

<template>
  <svg
    :width="size" :height="size" viewBox="0 0 24 24"
    fill="none" stroke="currentColor" stroke-width="2"
    stroke-linecap="round" stroke-linejoin="round"
    aria-hidden="true" class="inline-block shrink-0"
    v-html="inner"
  />
</template>
