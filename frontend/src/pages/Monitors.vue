<script setup>
import { ref, onMounted, onUnmounted } from 'vue'
import { useRoute } from 'vue-router'
import AppShell from '../components/AppShell.vue'
import { api } from '../lib/api'

const route = useRoute()
const selectedNsName = () => {
  const sel = (route.query.ns || '').split(',').filter(Boolean)
  return sel.length === 1 ? sel[0] : null
}

const monitors = ref([])
const namespaces = ref([])
const loading = ref(true)
const err = ref('')
let timer = null

const KINDS = [
  { v: 'http', label: 'HTTP', ph: 'https://example.com/health' },
  { v: 'tcp', label: 'TCP', ph: 'host:port' },
  { v: 'ping', label: 'Ping', ph: 'host or IP' },
  { v: 'keyword', label: 'Keyword', ph: 'https://example.com' },
]
const kindLabel = (k) => KINDS.find((x) => x.v === k)?.label || k

async function load() {
  try { monitors.value = await api.get('/api/monitors'); err.value = '' }
  catch { if (!monitors.value.length) err.value = 'Failed to load monitors' }
  loading.value = false
}

const showAdd = ref(false)
const nm = ref({ name: '', kind: 'http', target: '', interval_secs: 60, nsId: '' })
const addErr = ref('')
const kindPh = () => KINDS.find((x) => x.v === nm.value.kind)?.ph

async function addMonitor() {
  addErr.value = ''
  if (!nm.value.name.trim() || !nm.value.target.trim()) { addErr.value = 'Name and target are required.'; return }
  if (!nm.value.nsId) { addErr.value = 'Pick a namespace.'; return }
  try {
    await api.post(`/api/namespaces/${nm.value.nsId}/monitors`, {
      name: nm.value.name.trim(), kind: nm.value.kind, target: nm.value.target.trim(), interval_secs: Number(nm.value.interval_secs) || 60,
    })
    showAdd.value = false
    nm.value = { name: '', kind: 'http', target: '', interval_secs: 60, nsId: nm.value.nsId }
    await load()
  } catch (e) { addErr.value = e.status === 403 ? 'You need editor access to that namespace.' : `Failed (${e.status}).` }
}
async function removeMonitor(m) {
  if (!confirm(`Delete monitor "${m.name}"?`)) return
  try { await api.del(`/api/monitors/${m.id}`); await load() } catch (e) { alert(`Failed (${e.status}).`) }
}

const statusOf = (m) => (m.up == null ? 'pending' : m.up ? 'up' : 'down')
const fmtAgo = (t) => {
  if (!t) return 'never'
  const s = Math.round((Date.now() - new Date(t).getTime()) / 1000)
  return s < 60 ? `${s}s ago` : s < 3600 ? `${Math.round(s / 60)}m ago` : `${Math.round(s / 3600)}h ago`
}

onMounted(async () => {
  try {
    namespaces.value = await api.get('/api/namespaces')
    const match = namespaces.value.find((n) => n.name === selectedNsName())
    nm.value.nsId = (match || namespaces.value[0])?.id || ''
  } catch {}
  await load()
  timer = setInterval(load, 10000)
})
onUnmounted(() => clearInterval(timer))
</script>

<template>
  <AppShell title="Services">
    <div class="mx-auto max-w-5xl space-y-4">
      <div class="flex items-center justify-between gap-3">
        <p class="text-sm text-muted">Service checks — HTTP / TCP / ping / keyword. Status comes from the latest heartbeat.</p>
        <button @click="showAdd = !showAdd" class="flex shrink-0 items-center gap-1.5 rounded-lg bg-accent px-3.5 py-2 text-sm font-semibold text-accentfg hover:opacity-90">
          <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M12 5v14M5 12h14"/></svg> Add monitor
        </button>
      </div>

      <!-- create -->
      <form v-if="showAdd" @submit.prevent="addMonitor" class="space-y-2 rounded-xl border border-line bg-surface p-4">
        <div class="flex flex-wrap gap-2">
          <input v-model="nm.name" placeholder="name (e.g. API health)" class="min-w-44 flex-1 rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg placeholder:text-faint focus:border-accent/60 focus:outline-none" />
          <select v-model="nm.kind" class="rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none">
            <option v-for="k in KINDS" :key="k.v" :value="k.v">{{ k.label }}</option>
          </select>
          <select v-model="nm.nsId" class="rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none">
            <option v-for="n in namespaces" :key="n.id" :value="n.id">{{ n.name }}</option>
          </select>
        </div>
        <div class="flex flex-wrap gap-2">
          <input v-model="nm.target" :placeholder="kindPh()" class="min-w-56 flex-1 rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg placeholder:text-faint focus:border-accent/60 focus:outline-none" />
          <div class="flex items-center gap-1.5"><input v-model.number="nm.interval_secs" type="number" min="5" class="w-20 rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none" /><span class="text-xs text-muted">sec</span></div>
          <button type="submit" class="shrink-0 rounded-lg bg-accent px-4 py-2 text-sm font-medium text-accentfg hover:opacity-90">Create</button>
        </div>
        <p v-if="addErr" class="text-xs text-rose-400">{{ addErr }}</p>
      </form>

      <p v-if="loading" class="text-sm text-muted">Loading…</p>
      <p v-else-if="err" class="text-sm text-rose-400">{{ err }}</p>
      <p v-else-if="!monitors.length" class="rounded-xl border border-line bg-surface p-6 text-center text-sm text-muted">No monitors yet. Add a service check above.</p>

      <div v-else class="overflow-hidden rounded-xl border border-line bg-surface">
        <table class="w-full text-sm">
          <thead><tr class="border-b border-line text-left text-[11px] uppercase tracking-wider text-faint">
            <th class="px-4 py-3 font-medium">Status</th>
            <th class="px-4 py-3 font-medium">Name</th>
            <th class="px-4 py-3 font-medium">Type</th>
            <th class="px-4 py-3 font-medium">Target</th>
            <th class="px-4 py-3 font-medium text-right">Latency</th>
            <th class="px-4 py-3 font-medium text-right">Last check</th>
            <th class="px-4 py-3"></th>
          </tr></thead>
          <tbody>
            <tr v-for="m in monitors" :key="m.id" class="border-b border-line/60 last:border-0 hover:bg-surface2/50">
              <td class="px-4 py-3">
                <span class="inline-flex items-center gap-1.5 text-xs font-medium"
                  :class="statusOf(m) === 'up' ? 'text-accent' : statusOf(m) === 'down' ? 'text-red-500' : 'text-muted'">
                  <span class="h-2 w-2 rounded-full" :class="statusOf(m) === 'up' ? 'bg-accent' : statusOf(m) === 'down' ? 'bg-red-500' : 'bg-faint'"></span>
                  {{ statusOf(m) === 'up' ? 'Up' : statusOf(m) === 'down' ? 'Down' : 'Pending' }}
                </span>
              </td>
              <td class="px-4 py-3 text-fg">{{ m.name }}<div v-if="m.message" class="text-xs text-faint">{{ m.message }}</div></td>
              <td class="px-4 py-3 text-muted">{{ kindLabel(m.kind) }}</td>
              <td class="px-4 py-3 font-mono text-xs text-muted">{{ m.target }}</td>
              <td class="px-4 py-3 text-right tabular-nums text-muted">{{ m.latency_ms != null ? m.latency_ms + ' ms' : '—' }}</td>
              <td class="px-4 py-3 text-right tabular-nums text-muted">{{ fmtAgo(m.last_check) }}</td>
              <td class="px-4 py-3 text-right">
                <button @click="removeMonitor(m)" title="Delete monitor" class="text-muted hover:text-rose-400">
                  <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2m3 0v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/></svg>
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </AppShell>
</template>
