<script setup>
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import AppShell from '../components/AppShell.vue'
import PageLoader from '../components/PageLoader.vue'
import { api } from '../lib/api'
import { minLoad } from '../lib/minLoad'

const route = useRoute()
const router = useRouter()
const editId = computed(() => route.params.id || null)

const namespaces = ref([])
const monitors = ref([])
const systems = ref([])
const channels = ref([]) // global channel list { id, name, kind, namespace }
const loaded = ref(false)
const err = ref('')
const saving = ref(false)

const METRIC_LABEL = { cpu_percent: 'CPU %', mem_percent: 'Memory %', load1: 'Load 1m' }
const ed = ref({ srcType: 'monitor', targetId: '', scopeNs: '', condType: 'down', metric: 'cpu_percent', op: '>', value: 90, offlineSecs: 120, channels: new Set(), renotify: '' })

const isScope = computed(() => ed.value.srcType === 'all_services' || ed.value.srcType === 'all_hosts')
const isServiceLike = computed(() => ed.value.srcType === 'monitor' || ed.value.srcType === 'all_services')
const targetNs = computed(() => {
  const list = ed.value.srcType === 'monitor' ? monitors.value : systems.value
  const name = list.find((x) => x.id === ed.value.targetId)?.namespace
  return namespaces.value.find((n) => n.name === name) || null
})
const saveNs = computed(() => (isScope.value ? namespaces.value.find((n) => n.id === ed.value.scopeNs) || null : targetNs.value))
const candidates = computed(() => (ed.value.srcType === 'all_services' ? monitors.value : systems.value).filter((x) => x.namespace === saveNs.value?.name))

function setSrcType(t) {
  ed.value.srcType = t
  ed.value.targetId = ''
  ed.value.condType = t === 'monitor' || t === 'all_services' ? 'down' : 'metric'
}
function toggleChan(id) {
  const s = ed.value.channels
  s.has(id) ? s.delete(id) : s.add(id)
  ed.value.channels = new Set(s)
}

const targetName = computed(() => {
  if (ed.value.srcType === 'all_services') return 'any service'
  if (ed.value.srcType === 'all_hosts') return 'any host'
  const list = ed.value.srcType === 'monitor' ? monitors.value : systems.value
  return list.find((x) => x.id === ed.value.targetId)?.name || ''
})
const condText = computed(() => {
  if (isServiceLike.value) return 'is DOWN'
  if (ed.value.condType === 'offline') return `offline > ${ed.value.offlineSecs}s`
  return `${METRIC_LABEL[ed.value.metric]} ${ed.value.op} ${ed.value.value}`
})

// ---- per-channel test (works before the rule is saved) ----
const testState = ref({})
async function testChan(id) {
  testState.value = { ...testState.value, [id]: 'run' }
  try { await api.post(`/api/channels/${id}/test`); testState.value = { ...testState.value, [id]: 'ok' } }
  catch { testState.value = { ...testState.value, [id]: 'fail' } }
  setTimeout(() => { testState.value = { ...testState.value, [id]: undefined } }, 3000)
}

function buildCondition() {
  if (isServiceLike.value) return {}
  if (ed.value.condType === 'offline') return { offline_secs: Number(ed.value.offlineSecs) || 120 }
  return { metric: ed.value.metric, op: ed.value.op, value: Number(ed.value.value) }
}
function backToList() { router.push({ name: 'alerts', query: route.query.ns ? { ns: route.query.ns } : {} }) }

async function save() {
  err.value = ''
  if (!editId.value && !isScope.value && !ed.value.targetId) { err.value = `Pick a ${ed.value.srcType === 'monitor' ? 'service' : 'host'}.`; return }
  if (!ed.value.channels.size) { err.value = 'Pick at least one channel.'; return }
  const channel_ids = [...ed.value.channels]
  const renotify_secs = ed.value.renotify ? Number(ed.value.renotify) : null
  saving.value = true
  try {
    if (editId.value) {
      await api.patch(`/api/alerts/${editId.value}`, { channel_ids, renotify_secs, condition: buildCondition() })
    } else {
      if (!saveNs.value) { err.value = 'Pick a source first.'; saving.value = false; return }
      const body = { channel_ids, renotify_secs, condition: buildCondition() }
      if (isScope.value) body.scope_kind = ed.value.srcType
      else if (ed.value.srcType === 'monitor') body.monitor_id = ed.value.targetId
      else body.system_id = ed.value.targetId
      await api.post(`/api/namespaces/${saveNs.value.id}/alerts`, body)
    }
    backToList()
  } catch (e) { err.value = e.status === 403 ? 'You need editor access.' : `Failed (${e.status}).` }
  finally { saving.value = false }
}

onMounted(async () => {
  const work = (async () => {
    const [ns, mons, sys, chs] = await Promise.all([
      api.get('/api/namespaces').catch(() => []),
      api.get('/api/monitors').catch(() => []),
      api.get('/api/systems').catch(() => []),
      api.get('/api/channels').catch(() => []),
    ])
    namespaces.value = ns; monitors.value = mons; systems.value = sys; channels.value = chs
    if (editId.value) {
      const a = await api.get(`/api/alerts/${editId.value}`)
      const c = a.condition || {}
      const serviceLike = a.scope_kind === 'all_services' || (!a.scope_kind && a.monitor_id)
      ed.value = {
        srcType: a.scope_kind || (a.monitor_id ? 'monitor' : 'host'),
        targetId: a.monitor_id || a.system_id || '',
        scopeNs: a.scope_namespace_id || '',
        condType: serviceLike ? 'down' : c.offline_secs ? 'offline' : 'metric',
        metric: c.metric || 'cpu_percent', op: c.op || '>', value: c.value ?? 90, offlineSecs: c.offline_secs ?? 120,
        channels: new Set((a.channels || []).map((ch) => ch.id)),
        renotify: a.renotify_secs ? String(a.renotify_secs) : '',
      }
    } else {
      ed.value.scopeNs = ns[0]?.id || ''
    }
  })()
  await minLoad(work)
  loaded.value = true
})
</script>

<template>
  <AppShell :breadcrumb="[{ label: 'Rules', to: { name: 'alerts', query: route.query.ns ? { ns: route.query.ns } : {} } }, { label: editId ? 'Edit rule' : 'New rule' }]">
    <PageLoader v-if="!loaded" />
    <template v-else>
      <div class="grid gap-4 lg:grid-cols-[1fr_320px]">
        <!-- form -->
        <div class="overflow-hidden rounded-2xl border border-line bg-surface">
          <!-- 1. source -->
          <div class="border-b border-line p-5">
            <div class="mb-2.5 flex items-center gap-2 text-[11px] font-semibold uppercase tracking-wide text-faint"><span class="grid h-[18px] w-[18px] place-items-center rounded bg-surface2 text-accent">1</span>What to watch</div>
            <div v-if="!editId" class="mb-2.5 flex flex-wrap overflow-hidden rounded-lg border border-line">
              <button v-for="o in [['monitor','Service'],['all_services','All services'],['host','Host'],['all_hosts','All hosts']]" :key="o[0]"
                @click="setSrcType(o[0])" class="px-3.5 py-2 text-sm" :class="ed.srcType === o[0] ? 'bg-surface2 text-fg' : 'text-muted hover:text-fg'">{{ o[1] }}</button>
            </div>
            <UiSelect v-if="!editId && !isScope" v-model="ed.targetId" block
              :placeholder="`— pick a ${ed.srcType === 'monitor' ? 'service' : 'host'} —`"
              :options="(ed.srcType === 'monitor' ? monitors : systems).map((m) => ({ value: m.id, label: `${m.name} · ${m.namespace}` }))" />
            <div v-else-if="!editId">
              <UiSelect v-model="ed.scopeNs" block placeholder="— pick a namespace —" :options="namespaces.map((n) => ({ value: n.id, label: n.name }))" />
              <p class="mt-1.5 text-xs text-faint">Covers every {{ ed.srcType === 'all_services' ? 'service' : 'host' }} in this namespace — new ones included automatically.</p>
            </div>
            <div v-else class="rounded-lg border border-line bg-surface2 px-3 py-2.5 text-sm text-muted">Source can't be changed — delete and recreate to retarget.</div>
          </div>

          <!-- 2. condition -->
          <div class="border-b border-line p-5">
            <div class="mb-2.5 flex items-center gap-2 text-[11px] font-semibold uppercase tracking-wide text-faint"><span class="grid h-[18px] w-[18px] place-items-center rounded bg-surface2 text-accent">2</span>When it fires</div>
            <div v-if="isServiceLike" class="flex items-center gap-2 text-sm text-muted">
              Fires when {{ ed.srcType === 'all_services' ? 'any service' : 'the service' }} is <span class="rounded-md border border-amber-400/40 bg-amber-400/10 px-2 py-1 font-semibold text-amber-400">DOWN</span>
            </div>
            <div v-else class="flex flex-wrap items-center gap-2.5">
              <span class="text-sm text-muted">Fires when</span>
              <UiSelect v-model="ed.condType" :options="[['metric', 'a metric'], ['offline', 'it goes offline']]" />
              <template v-if="ed.condType === 'metric'">
                <UiSelect v-model="ed.metric" :options="[['cpu_percent', 'CPU %'], ['mem_percent', 'Memory %'], ['load1', 'Load 1m']]" />
                <UiSelect v-model="ed.op" :options="['>', '>=', '<', '<=']" />
                <input v-model.number="ed.value" type="number" class="w-24 rounded-lg border border-line bg-surface2 px-3 py-2.5 text-sm text-fg focus:border-accent/60 focus:outline-none" />
              </template>
              <template v-else>
                <span class="text-sm text-muted">no sample for</span>
                <input v-model.number="ed.offlineSecs" type="number" class="w-24 rounded-lg border border-line bg-surface2 px-3 py-2.5 text-sm text-fg focus:border-accent/60 focus:outline-none" /><span class="text-sm text-muted">seconds</span>
              </template>
            </div>
          </div>

          <!-- 3. channels (test before save) -->
          <div class="border-b border-line p-5">
            <div class="mb-2.5 flex items-center gap-2 text-[11px] font-semibold uppercase tracking-wide text-faint"><span class="grid h-[18px] w-[18px] place-items-center rounded bg-surface2 text-accent">3</span>Notify these channels — test before you save</div>
            <p v-if="!channels.length" class="text-xs text-faint">No channels yet — create one under <b>Alert › Notify channel</b>.</p>
            <div v-else class="space-y-2">
              <div v-for="c in channels" :key="c.id" class="flex items-center gap-2 rounded-lg border px-3 py-2"
                :class="ed.channels.has(c.id) ? 'border-accent/60 bg-accent/8' : 'border-line bg-surface2'">
                <button @click="toggleChan(c.id)" class="flex min-w-0 flex-1 items-center gap-2 text-left">
                  <svg v-if="ed.channels.has(c.id)" class="h-4 w-4 shrink-0 text-accent" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4"><path d="M20 6 9 17l-5-5"/></svg>
                  <span v-else class="h-4 w-4 shrink-0 rounded border border-line"></span>
                  <span class="truncate text-sm text-fg">{{ c.name }}</span>
                  <span class="shrink-0 text-[11px] text-faint">{{ c.kind }} · {{ c.namespace }}</span>
                </button>
                <span v-if="testState[c.id] === 'ok'" class="text-xs text-accent">✓ sent</span>
                <span v-else-if="testState[c.id] === 'fail'" class="text-xs text-rose-400">✗ failed</span>
                <button @click="testChan(c.id)" :disabled="testState[c.id] === 'run'" class="shrink-0 rounded-lg border border-line bg-surface px-2.5 py-1 text-xs text-fg hover:border-accent/50 disabled:opacity-50">{{ testState[c.id] === 'run' ? 'Testing…' : 'Send test' }}</button>
              </div>
            </div>
          </div>

          <!-- 4. delivery -->
          <div class="p-5">
            <div class="mb-2.5 flex items-center gap-2 text-[11px] font-semibold uppercase tracking-wide text-faint"><span class="grid h-[18px] w-[18px] place-items-center rounded bg-surface2 text-accent">4</span>Delivery</div>
            <label class="block max-w-xs"><span class="mb-1.5 block text-xs text-faint">Re-notify while still firing</span>
              <UiSelect v-model="ed.renotify" block :options="[['', 'Off — notify once'], ['900', 'every 15 min'], ['1800', 'every 30 min'], ['3600', 'every hour']]" />
            </label>
          </div>

          <div class="flex items-center gap-2.5 border-t border-line bg-surface/60 px-5 py-3.5">
            <span v-if="err" class="text-xs text-rose-400">{{ err }}</span>
            <span class="ml-auto"></span>
            <button @click="backToList" class="rounded-lg px-3 py-2 text-sm text-muted hover:text-fg">Cancel</button>
            <button @click="save" :disabled="saving" class="rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-accentfg hover:opacity-90 disabled:opacity-50">{{ saving ? 'Saving…' : 'Save rule' }}</button>
          </div>
        </div>

        <!-- right rail: wiring -->
        <div class="rounded-2xl border border-line bg-surface p-4">
          <div class="mb-2 text-[11px] font-semibold uppercase tracking-wide text-faint">Wiring</div>
          <p class="text-[13px] leading-relaxed text-muted">When <b class="text-fg">{{ targetName || '<source>' }}</b><template v-if="isScope && saveNs"> in <b class="text-fg">{{ saveNs.name }}</b></template> <b class="text-fg">{{ condText }}</b>, notify
            <template v-if="ed.channels.size"><b v-for="(id, i) in [...ed.channels]" :key="id" class="text-fg">{{ channels.find((c) => c.id === id)?.name }}{{ i < ed.channels.size - 1 ? ', ' : '' }}</b></template>
            <b v-else class="text-rose-400">no channel yet</b>.
          </p>
          <template v-if="isScope && !editId">
            <div class="mb-2 mt-4 text-[11px] font-semibold uppercase tracking-wide text-faint">Covers {{ candidates.length }} {{ ed.srcType === 'all_services' ? 'services' : 'hosts' }}</div>
            <div class="max-h-64 space-y-1 overflow-y-auto">
              <div v-for="t in candidates" :key="t.id" class="truncate rounded-md bg-surface2 px-2 py-1 text-xs text-fg">{{ t.name }}</div>
              <p v-if="!candidates.length" class="text-xs text-faint">No {{ ed.srcType === 'all_services' ? 'services' : 'hosts' }} in this namespace yet.</p>
            </div>
          </template>
        </div>
      </div>
    </template>
  </AppShell>
</template>
