<script setup>
import { ref, computed, onMounted } from 'vue'
import AppShell from '../components/AppShell.vue'
import PageLoader from '../components/PageLoader.vue'
import { api } from '../lib/api'
import { minLoad } from '../lib/minLoad'
import { useAuth } from '../stores/auth'

const auth = useAuth()
const isAdmin = computed(() => !!auth.user?.is_admin)
const rows = ref([])
const loading = ref(true)

async function load() {
  loading.value = true
  try { rows.value = await minLoad(api.get('/api/audit')) } catch { rows.value = [] }
  loading.value = false
}
onMounted(() => { if (isAdmin.value) load() })

const fmt = (s) => { const d = new Date(s); return isNaN(d) ? s : d.toLocaleString([], { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false }) }
const methodColor = (m) => ({ POST: 'text-accent', PATCH: 'text-amber-400', PUT: 'text-amber-400', DELETE: 'text-red-400' }[m] || 'text-muted')
const statusColor = (s) => (s < 300 ? 'text-accent' : s < 400 ? 'text-amber-400' : 'text-red-400')

// Humanize method+path into a readable action so the log reads "Deleted notify
// channel" instead of "DELETE /api/channels/<uuid>". The raw path stays in its
// own column for investigation.
const VERB = { POST: 'Create', PATCH: 'Update', PUT: 'Update', DELETE: 'Delete' }
const ACTION = { test: 'Test', upload: 'Upload', restore: 'Restore', run: 'Run', revoke: 'Revoke' }
const ENTITY = {
  channels: 'Notify channel', alerts: 'Alert rule', monitors: 'Monitor', systems: 'System',
  namespaces: 'Namespace', users: 'User', keys: 'Enrollment token', tokens: 'Enrollment token',
  members: 'Member', memberships: 'Member', backup: 'Backup', schedule: 'Backup schedule',
  data: 'Data & retention', restore: 'Backup', s3: 'S3 backup',
}
const cap = (s) => (s ? s[0].toUpperCase() + s.slice(1) : s)
const isId = (s) => /^[0-9a-f]{8}-[0-9a-f]{4}/i.test(s) || /^\d+$/.test(s)
function describe(r) {
  const segs = r.path.replace(/^\/api\//, '').split('/').filter(Boolean)
  const names = segs.filter((s) => !isId(s))
  let verb = VERB[r.method] || r.method
  if (ACTION[names[names.length - 1]]) verb = ACTION[names.pop()]
  const key = names[names.length - 1]
  const entity = ENTITY[key] || cap(key) || 'Resource'
  // single string so Vue's whitespace condensing can't drop the space between words
  return { label: `${verb} ${entity}` }
}
const decorated = computed(() => rows.value.map((r) => ({ ...r, d: describe(r) })))
</script>

<template>
  <AppShell title="Audit">
    <div v-if="!isAdmin" class="rounded-xl border border-line bg-surface p-6 text-center text-muted">Only system admins can view the audit log.</div>
    <div v-else class="space-y-3">
      <div class="flex items-center gap-2">
        <p class="text-sm text-muted">Every change action (who · what · when · result). Newest first, last 500.</p>
        <button @click="load" class="ml-auto rounded-lg border border-line bg-surface2 px-3 py-1.5 text-xs text-muted hover:text-accent">Refresh</button>
      </div>
      <div class="overflow-hidden rounded-xl border border-line bg-surface">
        <table class="w-full text-sm">
          <thead><tr class="border-b border-line text-left text-[11px] uppercase tracking-wider text-faint">
            <th class="px-4 py-3 font-medium">When</th>
            <th class="px-4 py-3 font-medium">User</th>
            <th class="px-4 py-3 font-medium">Action</th>
            <th class="px-4 py-3 font-medium">Object</th>
            <th class="px-4 py-3 font-medium">Endpoint</th>
            <th class="px-4 py-3 font-medium text-right">Result</th>
          </tr></thead>
          <tbody>
            <tr v-if="loading"><td colspan="6"><PageLoader min-height="40vh" /></td></tr>
            <tr v-else-if="!rows.length"><td colspan="6" class="px-4 py-6 text-center text-muted">No actions logged yet.</td></tr>
            <tr v-for="(r, i) in decorated" :key="i" class="border-b border-line/60 last:border-0 hover:bg-surface2/50">
              <td class="px-4 py-2.5 tabular-nums text-muted">{{ fmt(r.at) }}</td>
              <td class="px-4 py-2.5 text-fg">{{ r.user_email || '—' }}</td>
              <td class="px-4 py-2.5 whitespace-nowrap font-medium" :class="methodColor(r.method)">{{ r.d.label }}</td>
              <td class="px-4 py-2.5 text-fg">{{ r.object_name || '—' }}</td>
              <td class="px-4 py-2.5 font-mono text-[11px] text-faint" :title="r.path">{{ r.path }}</td>
              <td class="px-4 py-2.5 text-right tabular-nums" :class="statusColor(r.status)">{{ r.status }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </AppShell>
</template>
