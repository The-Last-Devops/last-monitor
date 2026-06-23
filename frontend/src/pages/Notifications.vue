<script setup>
import { ref, watch, computed, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import AppShell from '../components/AppShell.vue'
import { api } from '../lib/api'

const route = useRoute()
const selectedNsName = () => {
  const sel = (route.query.ns || '').split(',').filter(Boolean)
  return sel.length === 1 ? sel[0] : null
}

const namespaces = ref([])
const nsId = ref('')
const channels = ref([])
const loading = ref(false)
const err = ref('')

// Each kind: its config fields (with placeholder + hint) and an emoji icon.
const KINDS = [
  { v: 'telegram', label: 'Telegram', icon: '✈️', fields: [
    { k: 'bot_token', label: 'Bot token', ph: '123456:ABC-DEF…', hint: 'From @BotFather' },
    { k: 'chat_id', label: 'Chat ID', ph: '-1001234567890', hint: 'Channel/group/user id' },
  ] },
  { v: 'slack', label: 'Slack', icon: '💬', fields: [
    { k: 'url', label: 'Incoming webhook URL', ph: 'https://hooks.slack.com/services/…', hint: 'Slack › Incoming Webhooks' },
  ] },
  { v: 'discord', label: 'Discord', icon: '🎮', fields: [
    { k: 'url', label: 'Webhook URL', ph: 'https://discord.com/api/webhooks/…', hint: 'Channel › Integrations › Webhooks' },
  ] },
  { v: 'webhook', label: 'Webhook', icon: '🔗', fields: [
    { k: 'url', label: 'URL', ph: 'https://example.com/hook', hint: 'Receives POST {"text": "…"}' },
  ] },
]
const kind = (k) => KINDS.find((x) => x.v === k)
const kindLabel = (k) => kind(k)?.label || k
const kindIcon = (k) => kind(k)?.icon || '🔔'

async function loadChannels() {
  if (!nsId.value) { channels.value = []; return }
  loading.value = true
  try { channels.value = await api.get(`/api/namespaces/${nsId.value}/channels`) } catch { channels.value = [] }
  loading.value = false
}
watch(nsId, loadChannels)

// create form
const nc = ref({ name: '', kind: 'telegram', config: {} })
watch(() => nc.value.kind, () => { nc.value.config = {} })
const curFields = computed(() => kind(nc.value.kind)?.fields || [])

async function addChannel() {
  err.value = ''
  if (!nc.value.name.trim()) { err.value = 'Give the channel a name.'; return }
  for (const f of curFields.value) if (!nc.value.config[f.k]) { err.value = `${f.label} is required.`; return }
  try {
    await api.post(`/api/namespaces/${nsId.value}/channels`, { name: nc.value.name.trim(), kind: nc.value.kind, config: nc.value.config })
    nc.value = { name: '', kind: nc.value.kind, config: {} }
    await loadChannels()
  } catch (e) { err.value = e.status === 403 ? 'You need editor access to this namespace.' : `Failed (${e.status}).` }
}
async function removeChannel(c) {
  if (!confirm(`Delete channel "${c.name}"? Alert rules using it are removed too.`)) return
  try { await api.del(`/api/channels/${c.id}`); await loadChannels() } catch (e) { alert(`Failed (${e.status}).`) }
}

// per-channel test + edit state
const testState = ref({}) // id -> 'testing' | 'ok' | 'fail'
async function testChannel(c) {
  testState.value = { ...testState.value, [c.id]: 'testing' }
  try { await api.post(`/api/channels/${c.id}/test`); testState.value = { ...testState.value, [c.id]: 'ok' } }
  catch { testState.value = { ...testState.value, [c.id]: 'fail' } }
  setTimeout(() => { testState.value = { ...testState.value, [c.id]: undefined } }, 3000)
}
const editing = ref(null) // channel id being edited
const editForm = ref({ name: '', config: {} })
function startEdit(c) {
  editing.value = c.id
  editForm.value = { name: c.name, config: { ...(c.config || {}) } }
}
async function saveEdit(c) {
  try { await api.patch(`/api/channels/${c.id}`, { name: editForm.value.name.trim(), config: editForm.value.config }); editing.value = null; await loadChannels() }
  catch (e) { alert(`Failed (${e.status}).`) }
}

onMounted(async () => {
  try { namespaces.value = await api.get('/api/namespaces') } catch { namespaces.value = [] }
  const match = namespaces.value.find((n) => n.name === selectedNsName())
  nsId.value = (match || namespaces.value[0])?.id || ''
})
</script>

<template>
  <AppShell title="Notify channels">
    <div class="max-w-3xl space-y-5">
      <div class="flex items-center gap-3">
        <h2 class="text-sm font-semibold text-fg">Channels</h2>
        <select v-model="nsId" class="rounded-lg border border-line bg-surface2 px-3 py-1.5 text-sm text-fg focus:border-accent/60 focus:outline-none">
          <option v-for="n in namespaces" :key="n.id" :value="n.id">{{ n.name }}</option>
        </select>
      </div>
      <p class="text-xs text-faint">Where alerts are delivered for this namespace. Add a channel, send a test, then attach it to alert rules.</p>

      <!-- create -->
      <form @submit.prevent="addChannel" class="space-y-3 rounded-xl border border-line bg-surface p-4">
        <div class="text-sm font-semibold text-fg">New channel</div>
        <!-- kind picker as tabs -->
        <div class="flex flex-wrap gap-2">
          <button v-for="k in KINDS" :key="k.v" type="button" @click="nc.kind = k.v"
            class="flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-sm"
            :class="nc.kind === k.v ? 'border-accent/60 bg-accent/10 text-accent' : 'border-line bg-surface2 text-muted hover:text-fg'">
            <span>{{ k.icon }}</span>{{ k.label }}
          </button>
        </div>
        <label class="block text-xs text-faint">Name<input v-model="nc.name" placeholder="e.g. ops-telegram" class="mt-1 block w-full rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg placeholder:text-faint focus:border-accent/60 focus:outline-none" /></label>
        <label v-for="f in curFields" :key="f.k" class="block text-xs text-faint">{{ f.label }}<span v-if="f.hint" class="ml-2 text-faint/70">· {{ f.hint }}</span>
          <input v-model="nc.config[f.k]" :placeholder="f.ph" class="mt-1 block w-full rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg placeholder:text-faint focus:border-accent/60 focus:outline-none" />
        </label>
        <div class="flex items-center gap-3">
          <button type="submit" class="rounded-lg bg-accent px-4 py-2 text-sm font-medium text-accentfg hover:opacity-90">Add channel</button>
          <span v-if="err" class="text-xs text-rose-400">{{ err }}</span>
        </div>
      </form>

      <!-- list -->
      <p v-if="loading" class="text-sm text-muted">Loading…</p>
      <p v-else-if="!channels.length" class="rounded-xl border border-line bg-surface p-6 text-center text-sm text-muted">No channels in this namespace yet.</p>
      <div v-else class="space-y-2">
        <div v-for="c in channels" :key="c.id" class="rounded-xl border border-line bg-surface p-3">
          <div class="flex items-center gap-3">
            <span class="grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-surface2 text-lg">{{ kindIcon(c.kind) }}</span>
            <div class="min-w-0 flex-1">
              <div class="truncate text-sm font-medium text-fg">{{ c.name }}</div>
              <div class="text-xs text-faint">{{ kindLabel(c.kind) }}</div>
            </div>
            <div class="flex shrink-0 items-center gap-2">
              <span v-if="testState[c.id] === 'ok'" class="text-xs text-accent">✓ sent</span>
              <span v-else-if="testState[c.id] === 'fail'" class="text-xs text-rose-400">✗ failed</span>
              <button @click="testChannel(c)" :disabled="testState[c.id] === 'testing'" class="rounded-lg border border-line bg-surface2 px-2.5 py-1 text-xs text-fg hover:border-accent/50 disabled:opacity-50">{{ testState[c.id] === 'testing' ? 'Testing…' : 'Test' }}</button>
              <button @click="editing === c.id ? (editing = null) : startEdit(c)" class="text-muted hover:text-accent" title="Edit">
                <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.1 2.1 0 0 1 3 3L12 15l-4 1 1-4Z"/></svg>
              </button>
              <button @click="removeChannel(c)" class="text-muted hover:text-rose-400" title="Delete">
                <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2m3 0v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/></svg>
              </button>
            </div>
          </div>
          <!-- inline edit -->
          <div v-if="editing === c.id" class="mt-3 space-y-2 border-t border-line/60 pt-3">
            <label class="block text-xs text-faint">Name<input v-model="editForm.name" class="mt-1 block w-full rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none" /></label>
            <label v-for="f in (kind(c.kind)?.fields || [])" :key="f.k" class="block text-xs text-faint">{{ f.label }}
              <input v-model="editForm.config[f.k]" :placeholder="f.ph" class="mt-1 block w-full rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg placeholder:text-faint focus:border-accent/60 focus:outline-none" />
            </label>
            <div class="flex gap-2">
              <button @click="saveEdit(c)" class="rounded-lg bg-accent px-3 py-1.5 text-sm font-medium text-accentfg hover:opacity-90">Save</button>
              <button @click="editing = null" class="text-sm text-muted hover:text-fg">Cancel</button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </AppShell>
</template>
