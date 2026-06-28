<script setup>
// Overview / Dashboard — attention-first landing. Incidents lead, KPI summary
// second, fleet CPU trend demoted to the bottom. Aggregates across all selected
// namespaces (?ns=a,b; empty = all), like Systems.vue.
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useRoute } from 'vue-router'
import AppShell from '../components/AppShell.vue'
import PageLoader from '../components/PageLoader.vue'
import IncidentList from '../components/IncidentList.vue'
import FleetCharts from '../components/FleetCharts.vue'
import VIcon from '../components/VIcon.vue'
import { api } from '../lib/api'
import { useCached } from '../lib/cache'
import { online, hostState, worstReason, ago, pct, DEFAULT_THR } from '../lib/triage'

const route = useRoute()
const selectedNs = computed(() => (route.query.ns || '').split(',').filter(Boolean))
const inNs = (s) => selectedNs.value.length === 0 || selectedNs.value.includes(s.namespace)

const systems = ref([])
const thresholds = ref({}) // namespace name -> thresholds
const namespaces = ref([])
const alerts = ref([]) // firing alert rows across selected namespaces
const fleet = ref(null)
let timer = null

const thrOf = (s) => thresholds.value[s.namespace] || DEFAULT_THR
const hosts = computed(() => systems.value.filter(inNs))
function avg(arr, f) { const v = arr.map(f).filter((x) => x != null); return v.length ? Math.round(v.reduce((a, b) => a + b, 0) / v.length) : null }

const summary = computed(() => {
  let up = 0, down = 0, warn = 0
  for (const s of hosts.value) {
    if (online(s)) up++
    const st = hostState(s, thrOf(s))
    if (st === 'down') down++
    else if (st === 'warn') warn++
  }
  return {
    total: hosts.value.length, up, down, warn,
    cpu: avg(hosts.value.filter(online), (s) => s.cpu_percent),
    mem: avg(hosts.value.filter(online), (s) => pct(s.mem_used, s.mem_total)),
  }
})
const firing = computed(() => alerts.value.filter((a) => a.enabled && a.firing === true))

// Incidents = each down/warn host + each firing alert (deduped: a host already
// covered by a host-scoped firing alert still gets one row from the alert).
const incidents = computed(() => {
  const out = []
  for (const s of hosts.value) {
    const st = hostState(s, thrOf(s))
    if (st === 'ok') continue
    const reason = online(s)
      ? `${worstReason(s, thrOf(s)) || 'over threshold'} · ${ago(s.last_seen)}`
      : `offline · ${ago(s.last_seen)}`
    out.push({ id: 'h:' + s.id, tone: st, host: s.name, reason, ns: s.namespace, systemId: s.id })
  }
  for (const a of firing.value) {
    const dur = a.since ? ' · ' + ago(a.since) : ''
    out.push({
      id: 'a:' + a.id,
      tone: 'down',
      host: a.target_name || 'alert',
      reason: `${condText(a)} firing${dur}`,
      ns: a.namespace,
      systemId: a.system_id || null,
    })
  }
  // down before warn, then by host name
  return out.sort((x, y) => (x.tone === y.tone ? x.host.localeCompare(y.host) : x.tone === 'down' ? -1 : 1))
})

const METRIC_LABEL = { cpu_percent: 'CPU %', mem_percent: 'Memory %', load1: 'Load 1m' }
function condText(a) {
  const c = a.condition || {}
  if (a.target_kind === 'monitor' || a.target_kind === 'all_services') return 'service down'
  if (c.offline_secs) return `offline > ${c.offline_secs}s`
  if (c.metric) return `${METRIC_LABEL[c.metric] || c.metric} ${c.op} ${c.value}`
  return 'alert'
}

// ---- fleet avg CPU trend (24h) ----
const FSPAN = 86400
const trendCharts = computed(() => {
  const f = fleet.value
  if (!f || !f.t || !f.t.length) return []
  const visible = new Set(hosts.value.map((s) => s.name))
  const series = (f.cpu || []).filter((s) => visible.has(s.name))
  if (!series.length) return []
  // average CPU across visible hosts per timestamp
  const data = f.t.map((_, i) => {
    const vals = series.map((s) => s.data[i]).filter((v) => v != null)
    return vals.length ? vals.reduce((a, b) => a + b, 0) / vals.length : null
  })
  return [{ title: 'Avg CPU · last 24h', unit: '%', series: [{ name: 'fleet avg', color: 'rgb(var(--accent))', data }] }]
})

const { loaded, reload: load } = useCached({
  key: () => 'overview:' + selectedNs.value.join(','),
  load: async () => {
    const nss = namespaces.value
    const sel = selectedNs.value.length ? nss.filter((n) => selectedNs.value.includes(n.name)) : nss
    const [sys, thr, fl, alertLists] = await Promise.all([
      api.get('/api/systems').catch(() => []),
      api.get('/api/thresholds').catch(() => []),
      api.get('/api/fleet?range=24h').catch(() => null),
      Promise.all(sel.map((n) =>
        api.get(`/api/namespaces/${n.id}/alerts`)
          .then((rows) => rows.map((x) => ({ ...x, namespace: n.name })))
          .catch(() => []),
      )),
    ])
    const tm = {}; for (const x of thr) tm[x.namespace] = x
    const seen = new Set()
    const al = alertLists.flat().filter((a) => !seen.has(a.id) && seen.add(a.id))
    return { systems: sys, thresholds: tm, alerts: al, fleet: fl }
  },
  apply: (d) => { systems.value = d.systems; thresholds.value = d.thresholds; alerts.value = d.alerts; fleet.value = d.fleet },
})
watch(() => route.query.ns, load)
onMounted(async () => {
  try { namespaces.value = await api.get('/api/namespaces') } catch {}
  await load()
  timer = setInterval(load, 10000)
})
onUnmounted(() => clearInterval(timer))
</script>

<template>
  <AppShell title="Infrastructure">
    <template #title-after>
      <span class="text-xs text-muted">
        {{ summary.total }} hosts
        <span v-if="summary.down" class="text-down"> · {{ summary.down }} down</span>
        <span v-if="summary.warn" class="text-warn"> · {{ summary.warn }} warning</span>
      </span>
    </template>

    <PageLoader v-if="!loaded" />
    <div v-else class="space-y-5">
      <!-- 1) Needs attention FIRST -->
      <IncidentList :incidents="incidents" />

      <!-- 2) KPI strip -->
      <section class="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <div class="rounded-xl border border-line bg-surface p-4">
          <div class="text-xs uppercase tracking-wider text-muted">Hosts up</div>
          <div class="mt-1 font-mono text-metric font-extrabold tabular-nums text-ok">{{ summary.up }}<span class="text-h2 text-faint">/{{ summary.total }}</span></div>
        </div>
        <div class="rounded-xl border border-line bg-surface p-4">
          <div class="text-xs uppercase tracking-wider text-muted">Avg CPU</div>
          <div class="mt-1 font-mono text-metric font-extrabold tabular-nums text-fg">{{ summary.cpu ?? '—' }}<span class="text-h2 text-faint">%</span></div>
        </div>
        <div class="rounded-xl border border-line bg-surface p-4">
          <div class="text-xs uppercase tracking-wider text-muted">Memory</div>
          <div class="mt-1 font-mono text-metric font-extrabold tabular-nums text-fg">{{ summary.mem ?? '—' }}<span class="text-h2 text-faint">%</span></div>
        </div>
        <div class="rounded-xl border p-4" :class="firing.length ? 'border-down/38 bg-down/12' : 'border-line bg-surface'">
          <div class="text-xs uppercase tracking-wider text-muted">Firing</div>
          <div class="mt-1 font-mono text-metric font-extrabold tabular-nums" :class="firing.length ? 'text-down' : 'text-fg'">{{ firing.length }}</div>
        </div>
      </section>

      <!-- 3) Trend LAST (demoted) -->
      <section class="rounded-xl border border-line bg-surface p-4">
        <div class="mb-2 flex items-center gap-2">
          <VIcon name="metrics" :size="16" class="text-muted" />
          <h2 class="text-h2 font-semibold text-fg">Fleet trend</h2>
        </div>
        <FleetCharts v-if="trendCharts.length" :charts="trendCharts" :time="fleet?.t || []" :span-seconds="FSPAN" sync-key="overview-trend" />
        <p v-else class="py-6 text-center text-xs text-muted">No metrics yet.</p>
      </section>
    </div>
  </AppShell>
</template>
