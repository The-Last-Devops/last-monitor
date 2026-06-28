<script setup>
// Presentational status-change feed for service monitors. Rows are passed in
// pre-shaped by the parent (it owns fetching + namespace filtering); formatting
// helpers come in as props so this stays purely presentational.
defineProps({
  events: { type: Array, default: () => [] },
  evTime: { type: Function, required: true },
  evMessage: { type: Function, required: true },
  // (i) => { secs, ongoing } — duration the event's state lasted
  stateDur: { type: Function, required: true },
  fmtDur: { type: Function, required: true },
  // whether to show the per-event service name + link (list view), off on detail
  showService: { type: Boolean, default: true },
})
</script>

<template>
  <div class="overflow-hidden rounded-xl border border-line bg-surface">
    <div class="flex items-center gap-2 border-b border-line2 bg-head px-4 py-2.5">
      <VIcon name="pulse" :size="16" class="text-faint" />
      <h2 class="text-xs font-extrabold uppercase tracking-wide text-fg">Recent events</h2>
      <span class="rounded-pill bg-surface2 px-2 py-0.5 text-micro text-muted">{{ events.length }}</span>
    </div>
    <p v-if="!events.length" class="px-4 py-8 text-center text-sm text-muted">No status changes recorded recently.</p>
    <ul v-else class="divide-y divide-line">
      <li v-for="(e, i) in events" :key="i" class="flex items-center gap-3 px-4 py-2.5 hover:bg-hover">
        <StatePill :tone="e.up ? 'ok' : 'down'" :label="e.up ? 'Up' : 'Down'" />
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2">
            <RouterLink v-if="showService" :to="{ name: 'monitor', params: { id: e.monitor_id } }"
              class="truncate font-mono text-sm text-fg hover:text-accent" @click.stop>{{ e.name }}</RouterLink>
            <span class="truncate text-sm text-muted">{{ evMessage(e) }}</span>
          </div>
          <div class="mt-0.5 flex items-center gap-1.5 text-micro tabular-nums text-faint">
            <span>{{ evTime(e.at) }}</span>
            <span>·</span>
            <span>{{ fmtDur(stateDur(i).secs) }}<span v-if="stateDur(i).ongoing"> · ongoing</span></span>
          </div>
        </div>
      </li>
    </ul>
  </div>
</template>
