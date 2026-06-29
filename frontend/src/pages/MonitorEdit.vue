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
const loaded = ref(false)
const formErr = ref('')
const saving = ref(false)

const KINDS = [
  { v: 'http', label: 'HTTP(s)', ph: 'https://example.com/health' },
  { v: 'keyword', label: 'HTTP keyword', ph: 'https://example.com' },
  { v: 'tcp', label: 'TCP port', ph: 'host:port' },
  { v: 'ping', label: 'Ping', ph: 'host or IP' },
  { v: 'postgres', label: 'PostgreSQL', ph: 'postgres://user:pass@host:5432/db' },
  { v: 'mysql', label: 'MySQL', ph: 'mysql://user:pass@host:3306/db' },
  { v: 'mongodb', label: 'MongoDB', ph: 'mongodb://user:pass@host:27017' },
  { v: 'redis', label: 'Redis', ph: 'host:6379' },
  { v: 'rabbitmq', label: 'RabbitMQ', ph: 'host:5672' },
  { v: 'dns', label: 'DNS', ph: 'example.com' },
  { v: 'tls', label: 'TLS cert', ph: 'host:443' },
  { v: 'push', label: 'Push (passive)', ph: '' },
]
const isHttp = (k) => k === 'http' || k === 'keyword'
const isEdit = computed(() => editId.value != null)

const blank = () => ({
  id: null, name: '', kind: 'http', target: '', nsId: '', interval_secs: 60, timeout_secs: 15, retries: 1, upside_down: false,
  method: 'GET', accepted_status: '', max_redirects: 10, ignore_tls: false, headersText: '', body: '',
  authType: 'none', authUser: '', authPass: '', authToken: '', keyword: '', keyword_invert: false,
  password: '', expected_ip: '', cert_warn_days: 14, tags: '', description: '',
})
const f = ref(blank())

function buildConfig() {
  const v = f.value
  const cfg = {
    timeout_secs: Number(v.timeout_secs) || 15, retries: Number(v.retries) || 0, upside_down: v.upside_down,
    tags: v.tags.split(',').map((s) => s.trim()).filter(Boolean), description: v.description.trim(),
  }
  if (isHttp(v.kind)) {
    cfg.method = v.method
    cfg.accepted_status = v.accepted_status.trim()
    cfg.max_redirects = Number(v.max_redirects) || 0
    cfg.ignore_tls = v.ignore_tls
    const headers = {}
    for (const line of v.headersText.split('\n')) { const i = line.indexOf(':'); if (i > 0) headers[line.slice(0, i).trim()] = line.slice(i + 1).trim() }
    if (Object.keys(headers).length) cfg.headers = headers
    if (v.body.trim()) cfg.body = v.body
    if (v.authType === 'basic') cfg.auth = { type: 'basic', username: v.authUser, password: v.authPass }
    else if (v.authType === 'bearer') cfg.auth = { type: 'bearer', token: v.authToken }
  }
  if (v.kind === 'keyword') { cfg.keyword = v.keyword; cfg.keyword_invert = v.keyword_invert }
  if (v.kind === 'redis' && v.password) cfg.password = v.password
  if (v.kind === 'dns' && v.expected_ip.trim()) cfg.expected_ip = v.expected_ip.trim()
  if (v.kind === 'tls') cfg.cert_warn_days = Number(v.cert_warn_days) || 14
  return cfg
}

function back() { router.push({ name: 'monitors', query: route.query.ns ? { ns: route.query.ns } : {} }) }

async function submit() {
  formErr.value = ''
  const v = f.value
  if (!v.name.trim()) { formErr.value = 'Name is required.'; return }
  if (v.kind !== 'push' && !v.target.trim()) { formErr.value = 'Target is required.'; return }
  if (v.kind === 'keyword' && !v.keyword.trim()) { formErr.value = 'Keyword is required for keyword monitors.'; return }
  const target = v.kind === 'push' ? 'push' : v.target.trim()
  const config = buildConfig()
  saving.value = true
  try {
    if (isEdit.value) {
      await api.patch(`/api/monitors/${v.id}`, { name: v.name.trim(), target, interval_secs: Number(v.interval_secs) || 60, config })
    } else {
      if (!v.nsId) { formErr.value = 'Pick a namespace.'; saving.value = false; return }
      await api.post(`/api/namespaces/${v.nsId}/monitors`, { name: v.name.trim(), kind: v.kind, target, interval_secs: Number(v.interval_secs) || 60, config })
    }
    back()
  } catch (e) { formErr.value = e.status === 403 ? 'You need editor access to that namespace.' : `Failed (${e.status}).` }
  finally { saving.value = false }
}

onMounted(async () => {
  const work = (async () => {
    namespaces.value = await api.get('/api/namespaces').catch(() => [])
    if (editId.value) {
      const all = await api.get('/api/monitors').catch(() => [])
      const m = all.find((x) => x.id === editId.value)
      if (m) {
        const c = m.config || {}
        const auth = c.auth || {}
        f.value = {
          id: m.id, name: m.name, kind: m.kind, target: m.target, nsId: '', interval_secs: m.interval_secs,
          timeout_secs: c.timeout_secs ?? 15, retries: c.retries ?? 0, upside_down: !!c.upside_down,
          method: c.method || 'GET', accepted_status: c.accepted_status || '', max_redirects: c.max_redirects ?? 10, ignore_tls: !!c.ignore_tls,
          headersText: c.headers ? Object.entries(c.headers).map(([k, v]) => `${k}: ${v}`).join('\n') : '', body: c.body || '',
          authType: auth.type || 'none', authUser: auth.username || '', authPass: auth.password || '', authToken: auth.token || '',
          keyword: c.keyword || '', keyword_invert: !!c.keyword_invert,
          password: c.password || '', expected_ip: c.expected_ip || '', cert_warn_days: c.cert_warn_days ?? 14, tags: (c.tags || []).join(', '), description: c.description || '',
        }
      }
    } else {
      const pre = (route.query.ns || '').split(',').filter(Boolean)
      const match = namespaces.value.find((n) => n.name === pre[0])
      f.value.nsId = (match || namespaces.value[0])?.id || ''
    }
  })()
  await minLoad(work)
  loaded.value = true
})
</script>

<template>
  <AppShell :breadcrumb="[{ label: 'Services', to: { name: 'monitors', query: route.query.ns ? { ns: route.query.ns } : {} } }, { label: isEdit ? f.name : 'New service' }]">
    <PageLoader v-if="!loaded" />
    <template v-else>
      <form @submit.prevent="submit" class="max-w-3xl space-y-4 rounded-2xl border border-line bg-surface p-5">
        <div class="flex flex-wrap items-end gap-3">
          <label class="text-xs text-faint">Type<UiSelect v-model="f.kind" block :disabled="isEdit" class="mt-1" :options="KINDS.map((k) => ({ value: k.v, label: k.label }))" /></label>
          <label class="text-xs text-faint">Name<input v-model="f.name" placeholder="My service" class="mt-1 block w-64 rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg placeholder:text-faint focus:border-accent/60 focus:outline-none" /></label>
          <label v-if="!isEdit" class="text-xs text-faint">Namespace<UiSelect v-model="f.nsId" block class="mt-1" :options="namespaces.map((n) => ({ value: n.id, label: n.name }))" /></label>
        </div>
        <label v-if="f.kind !== 'push'" class="block text-xs text-faint">Target<input v-model="f.target" :placeholder="KINDS.find((k) => k.v === f.kind)?.ph" class="mt-1 block w-full rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg placeholder:text-faint focus:border-accent/60 focus:outline-none" /></label>
        <p v-else class="rounded-lg border border-line bg-surface2/40 px-3 py-2 text-xs text-muted">Passive check — a push URL is generated after you create it. Have your job call it on schedule; if no call arrives within the interval, it goes Down.</p>
        <label v-if="f.kind === 'tls'" class="block w-56 text-xs text-faint">Warn when expiring within (days)<input v-model.number="f.cert_warn_days" type="number" min="1" class="mt-1 block w-full rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none" /></label>

        <div class="flex flex-wrap gap-3">
          <label class="text-xs text-faint">Interval (s)<input v-model.number="f.interval_secs" type="number" min="5" class="mt-1 block w-24 rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none" /></label>
          <label class="text-xs text-faint">Timeout (s)<input v-model.number="f.timeout_secs" type="number" min="1" class="mt-1 block w-24 rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none" /></label>
          <label class="text-xs text-faint">Retries<input v-model.number="f.retries" type="number" min="0" class="mt-1 block w-24 rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none" /></label>
          <label class="flex items-center gap-2 self-end pb-2 text-sm text-fg"><input v-model="f.upside_down" type="checkbox" class="h-4 w-4" />Upside-down</label>
        </div>

        <div v-if="f.kind === 'keyword'" class="flex flex-wrap items-end gap-3">
          <label class="flex-1 text-xs text-faint">Keyword<input v-model="f.keyword" class="mt-1 block w-full rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none" /></label>
          <label class="flex items-center gap-2 pb-2 text-sm text-fg"><input v-model="f.keyword_invert" type="checkbox" class="h-4 w-4" />Invert (fail if present)</label>
        </div>
        <label v-if="f.kind === 'redis'" class="block w-72 text-xs text-faint">Password (optional)<input v-model="f.password" type="password" class="mt-1 block w-full rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none" /></label>
        <label v-if="f.kind === 'dns'" class="block w-72 text-xs text-faint">Expected IP (optional, substring)<input v-model="f.expected_ip" placeholder="1.2.3.4" class="mt-1 block w-full rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg placeholder:text-faint focus:border-accent/60 focus:outline-none" /></label>

        <details v-if="isHttp(f.kind)" open class="rounded-lg border border-line bg-surface2/40 p-3">
          <summary class="cursor-pointer text-xs uppercase tracking-wider text-faint">HTTP options</summary>
          <div class="mt-3 space-y-3">
            <div class="flex flex-wrap gap-3">
              <label class="text-xs text-faint">Method<UiSelect v-model="f.method" block class="mt-1" :options="['GET', 'POST', 'PUT', 'HEAD', 'DELETE', 'PATCH']" /></label>
              <label class="text-xs text-faint">Accepted status<input v-model="f.accepted_status" placeholder="200-299,301" class="mt-1 block w-40 rounded-lg border border-line bg-surface px-3 py-2 text-sm text-fg placeholder:text-faint focus:border-accent/60 focus:outline-none" /></label>
              <label class="text-xs text-faint">Max redirects<input v-model.number="f.max_redirects" type="number" min="0" class="mt-1 block w-24 rounded-lg border border-line bg-surface px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none" /></label>
              <label class="flex items-center gap-2 self-end pb-2 text-sm text-fg"><input v-model="f.ignore_tls" type="checkbox" class="h-4 w-4" />Ignore TLS errors</label>
            </div>
            <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
              <label class="block text-xs text-faint">Headers (one per line, <code>Key: Value</code>)<textarea v-model="f.headersText" rows="5" class="mt-1 block w-full rounded-lg border border-line bg-surface px-3 py-2 font-mono text-xs text-fg focus:border-accent/60 focus:outline-none"></textarea></label>
              <label class="block text-xs text-faint">Body<textarea v-model="f.body" rows="5" class="mt-1 block w-full rounded-lg border border-line bg-surface px-3 py-2 font-mono text-xs text-fg focus:border-accent/60 focus:outline-none"></textarea></label>
            </div>
            <div class="flex flex-wrap items-end gap-3">
              <label class="text-xs text-faint">Auth<UiSelect v-model="f.authType" block class="mt-1" :options="[['none', 'None'], ['basic', 'Basic'], ['bearer', 'Bearer']]" /></label>
              <template v-if="f.authType === 'basic'">
                <label class="text-xs text-faint">Username<input v-model="f.authUser" class="mt-1 block rounded-lg border border-line bg-surface px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none" /></label>
                <label class="text-xs text-faint">Password<input v-model="f.authPass" type="password" class="mt-1 block rounded-lg border border-line bg-surface px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none" /></label>
              </template>
              <label v-else-if="f.authType === 'bearer'" class="flex-1 text-xs text-faint">Token<input v-model="f.authToken" class="mt-1 block w-full rounded-lg border border-line bg-surface px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none" /></label>
            </div>
          </div>
        </details>

        <div class="flex flex-wrap gap-3">
          <label class="flex-1 text-xs text-faint">Tags (comma-separated)<input v-model="f.tags" placeholder="prod, api" class="mt-1 block w-full rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg placeholder:text-faint focus:border-accent/60 focus:outline-none" /></label>
          <label class="flex-1 text-xs text-faint">Description<input v-model="f.description" class="mt-1 block w-full rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none" /></label>
        </div>

        <div class="flex items-center gap-3 border-t border-line pt-4">
          <button type="submit" :disabled="saving" class="rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-accentfg hover:opacity-90 disabled:opacity-50">{{ saving ? 'Saving…' : isEdit ? 'Save changes' : 'Create service' }}</button>
          <button type="button" @click="back" class="text-sm text-muted hover:text-fg">Cancel</button>
          <span v-if="formErr" class="text-xs text-down">{{ formErr }}</span>
        </div>
      </form>
    </template>
  </AppShell>
</template>
