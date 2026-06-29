<script setup>
// Presentational "Down history" list of incidents for a service monitor.
// Incidents are computed by the parent (newest-first); formatting helpers are
// passed in so this component stays purely presentational. The list is capped at
// ~2/3 of the viewport (scrolls inside) and paginated when there are many, so a
// chatty service can't make the page scroll forever.
import { ref, computed, watch } from 'vue'

const props = defineProps({
  incidents: { type: Array, default: () => [] },
  evTime: { type: Function, required: true },
  durTxt: { type: Function, required: true },
})

const PAGE = 25
const page = ref(1)
const pages = computed(() => Math.max(1, Math.ceil(props.incidents.length / PAGE)))
const shown = computed(() => props.incidents.slice((page.value - 1) * PAGE, page.value * PAGE))
// reset to the first page if the incident set changes (e.g. range switch)
watch(() => props.incidents, () => { page.value = 1 })
</script>

<template>
  <div class="rounded-xl border border-line bg-surface p-4">
    <div class="mb-2 flex items-center gap-2">
      <span class="text-[11px] uppercase tracking-wider text-faint">Down history</span>
      <span v-if="incidents.length" class="text-[11px] text-faint">· {{ incidents.length }}</span>
    </div>
    <p v-if="!incidents.length" class="text-xs text-faint">No downtime in this range. 🎉</p>
    <template v-else>
      <!-- cap at ~2/3 of the viewport and scroll inside -->
      <ul class="max-h-[66vh] divide-y divide-line/60 overflow-y-auto">
        <li v-for="(it, i) in shown" :key="i" class="flex flex-wrap items-center gap-x-3 gap-y-1 py-2.5 text-sm">
          <span class="inline-flex items-center gap-1.5 font-medium" :class="it.ongoing ? 'text-down' : 'text-warn'">
            <span class="h-2 w-2 rounded-full" :class="it.ongoing ? 'bg-down' : 'bg-warn'"></span>
            {{ it.ongoing ? 'Down' : 'Resolved' }}
          </span>
          <span class="font-mono tabular-nums text-muted">{{ evTime(it.at) }}</span>
          <span class="text-faint">·</span>
          <span class="font-mono tabular-nums text-fg">{{ it.ongoing ? durTxt(Date.now() - it.start) + ' (ongoing)' : durTxt(it.end - it.start) }}</span>
          <span class="min-w-0 flex-1 truncate text-muted" v-tip="it.reason">{{ it.reason }}</span>
        </li>
      </ul>
      <!-- pager (only when more than one page) -->
      <div v-if="pages > 1" class="mt-3 flex items-center justify-between border-t border-line pt-3 text-xs">
        <button :disabled="page <= 1" @click="page--"
          class="rounded-lg border border-line px-2.5 py-1 text-muted hover:border-accent/50 hover:text-fg disabled:opacity-40">Prev</button>
        <span class="font-mono tabular-nums text-faint">Page {{ page }} / {{ pages }}</span>
        <button :disabled="page >= pages" @click="page++"
          class="rounded-lg border border-line px-2.5 py-1 text-muted hover:border-accent/50 hover:text-fg disabled:opacity-40">Next</button>
      </div>
    </template>
  </div>
</template>
