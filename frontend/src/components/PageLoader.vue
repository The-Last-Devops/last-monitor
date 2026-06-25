<script setup>
// Centered page loader — used wherever a page is fetching its first data, so the
// content area shows a tasteful spinner instead of a blank/empty flash.
defineProps({
  label: { type: String, default: 'Loading…' },
  // Vertical room the loader occupies; defaults to most of the viewport so it
  // sits in the middle of the content area rather than at the top.
  minHeight: { type: String, default: '60vh' },
})
</script>

<template>
  <div class="flex w-full flex-col items-center justify-center gap-4" :style="{ minHeight }">
    <span class="lm-spinner" aria-hidden="true"></span>
    <span v-if="label" class="text-sm text-muted">{{ label }}</span>
    <span class="sr-only">Loading</span>
  </div>
</template>

<style scoped>
.lm-spinner {
  width: 40px;
  height: 40px;
  border-radius: 9999px;
  border: 3px solid rgb(var(--line));
  border-top-color: rgb(var(--accent));
  animation: lm-spin 0.7s linear infinite;
  box-shadow: 0 0 22px -6px rgb(var(--accent) / 0.6);
}
@keyframes lm-spin {
  to { transform: rotate(360deg); }
}
@media (prefers-reduced-motion: reduce) {
  .lm-spinner { animation-duration: 1.6s; }
}
</style>
