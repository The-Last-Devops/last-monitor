<script setup>
import { onMounted, onBeforeUnmount } from 'vue'
import { RouterView } from 'vue-router'
import ConfirmDialog from './components/ConfirmDialog.vue'
import { updateAvailable, startBuildWatch } from './lib/buildWatch'

function reloadForUpdate() { location.reload() }

// The sidebar logo gently cycles hue via the --logo-hue CSS var. The favicon is
// NOT animated — it stays the fixed brand mark from /favicon.svg (linked in
// index.html), so the browser tab keeps a stable, recognizable icon.
let timer
onMounted(() => {
  const root = document.documentElement
  let hue = 170 // brand teal
  root.style.setProperty('--logo-hue', String(hue))
  // Honor reduced-motion: pick one color and stop.
  if (window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
  timer = setInterval(() => {
    hue = (hue + 5) % 360 // ~11s for a full cycle
    root.style.setProperty('--logo-hue', hue.toFixed(1))
  }, 150)
})
onBeforeUnmount(() => clearInterval(timer))

// Watch for a newer deployed build and offer a reload (see lib/buildWatch.js).
onMounted(startBuildWatch)
</script>

<template>
  <RouterView />
  <ConfirmDialog />

  <!-- a newer build was deployed while this tab was open → offer to reload -->
  <div v-if="updateAvailable"
    class="fixed inset-x-0 bottom-0 z-[60] flex flex-wrap items-center justify-center gap-3 border-t border-accent/40 bg-surface/95 px-4 py-2.5 text-sm backdrop-blur">
    <span class="text-fg">A new version of Vantage is available.</span>
    <button @click="reloadForUpdate"
      class="rounded-lg bg-accent px-3 py-1.5 text-xs font-semibold text-accentfg hover:opacity-90">Reload</button>
  </div>
</template>
