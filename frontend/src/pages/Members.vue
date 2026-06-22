<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import AppShell from '../components/AppShell.vue'
import { api } from '../lib/api'
import { useAuth } from '../stores/auth'

const auth = useAuth()
const isAdmin = computed(() => !!auth.user?.is_admin)

// ---- users (system roles) ----
const users = ref([])
const loading = ref(true)
async function loadUsers() {
  loading.value = true
  try { users.value = await api.get('/api/users') } catch { users.value = [] }
  loading.value = false
}

const SYS = [
  { v: 'user', label: 'User', desc: 'Access only the namespaces granted below' },
  { v: 'read_all', label: 'Admin · read-only', desc: 'View every namespace, no changes' },
  { v: 'admin', label: 'Admin', desc: 'Full access everywhere' },
]
const sysOf = (u) => (u.is_admin ? 'admin' : u.read_all ? 'read_all' : 'user')
async function setSys(u, v) {
  try { await api.patch(`/api/users/${u.id}`, { is_admin: v === 'admin', read_all: v === 'read_all' }); await loadUsers() }
  catch (e) { alert(e.status === 400 ? "You can't remove your own admin rights." : `Failed (${e.status}).`) }
}
async function removeUser(u) {
  if (!confirm(`Delete user ${u.email}? Their memberships and sessions are removed.`)) return
  try { await api.del(`/api/users/${u.id}`); await loadUsers() }
  catch (e) { alert(e.status === 400 ? "You can't delete yourself." : `Failed (${e.status}).`) }
}

// add user
const nu = ref({ email: '', password: '', role: 'user' })
const adding = ref(false)
const addErr = ref('')
async function addUser() {
  addErr.value = ''
  if (!nu.value.email.includes('@') || nu.value.password.length < 6) { addErr.value = 'Valid email and a password of 6+ chars.'; return }
  adding.value = true
  try {
    await api.post('/api/users', { email: nu.value.email, password: nu.value.password, is_admin: nu.value.role === 'admin', read_all: nu.value.role === 'read_all' })
    nu.value = { email: '', password: '', role: 'user' }
    await loadUsers()
  } catch (e) { addErr.value = e.status === 409 ? 'A user with that email already exists.' : `Failed (${e.status}).` }
  finally { adding.value = false }
}

// ---- per-namespace access ----
const namespaces = ref([])
const nsId = ref('')
const members = ref([])
const NS_ROLES = [{ v: 'viewer', label: 'Read' }, { v: 'editor', label: 'Write' }, { v: 'owner', label: 'Owner' }]
async function loadMembers() {
  if (!nsId.value) { members.value = []; return }
  try { members.value = await api.get(`/api/namespaces/${nsId.value}/members`) } catch { members.value = [] }
}
watch(nsId, loadMembers)

const nm = ref({ email: '', role: 'viewer' })
const mErr = ref('')
async function addMember() {
  mErr.value = ''
  if (!nm.value.email.includes('@')) { mErr.value = 'Enter the user’s email.'; return }
  try { await api.post(`/api/namespaces/${nsId.value}/members`, { email: nm.value.email, role: nm.value.role }); nm.value = { email: '', role: 'viewer' }; await loadMembers() }
  catch (e) { mErr.value = e.status === 404 ? 'No user with that email (create them first).' : `Failed (${e.status}).` }
}
async function setMemberRole(m, role) {
  try { await api.post(`/api/namespaces/${nsId.value}/members`, { email: m.email, role }); await loadMembers() } catch (e) { alert(`Failed (${e.status}).`) }
}
async function removeMember(m) {
  try { await api.del(`/api/namespaces/${nsId.value}/members/${m.user_id}`); await loadMembers() } catch (e) { alert(`Failed (${e.status}).`) }
}

onMounted(async () => {
  if (!isAdmin.value) return
  await loadUsers()
  try { namespaces.value = await api.get('/api/namespaces') } catch { namespaces.value = [] }
  if (namespaces.value[0]) nsId.value = namespaces.value[0].id
})
</script>

<template>
  <AppShell title="Members">
    <div v-if="!isAdmin" class="mx-auto max-w-md rounded-xl border border-line bg-surface p-6 text-center text-muted">
      Only system admins can manage members.
    </div>
    <div v-else class="mx-auto max-w-4xl space-y-8">
      <!-- users -->
      <section class="space-y-3">
        <h2 class="text-sm font-semibold text-fg">Users &amp; system roles</h2>
        <form @submit.prevent="addUser" class="flex flex-wrap items-start gap-2">
          <input v-model="nu.email" placeholder="email@company.com" class="min-w-48 flex-1 rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg placeholder:text-faint focus:border-accent/60 focus:outline-none" />
          <input v-model="nu.password" type="password" placeholder="password" class="w-40 rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg placeholder:text-faint focus:border-accent/60 focus:outline-none" />
          <select v-model="nu.role" class="rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none">
            <option v-for="r in SYS" :key="r.v" :value="r.v">{{ r.label }}</option>
          </select>
          <button type="submit" :disabled="adding" class="rounded-lg bg-accent px-4 py-2 text-sm font-medium text-accentfg hover:opacity-90 disabled:opacity-50">{{ adding ? 'Adding…' : 'Add user' }}</button>
        </form>
        <p v-if="addErr" class="text-xs text-rose-400">{{ addErr }}</p>

        <div class="overflow-hidden rounded-xl border border-line bg-surface">
          <table class="w-full text-sm">
            <thead><tr class="border-b border-line text-left text-[11px] uppercase tracking-wider text-faint">
              <th class="px-4 py-3 font-medium">Email</th>
              <th class="px-4 py-3 font-medium">System role</th>
              <th class="px-4 py-3 font-medium text-right">Namespaces</th>
              <th class="px-4 py-3"></th>
            </tr></thead>
            <tbody>
              <tr v-if="loading"><td colspan="4" class="px-4 py-6 text-center text-muted">Loading…</td></tr>
              <tr v-for="u in users" :key="u.id" class="border-b border-line/60 last:border-0 hover:bg-surface2/50">
                <td class="px-4 py-3 text-fg">{{ u.email }}<span v-if="u.id === auth.user?.id" class="ml-2 text-[10px] uppercase tracking-wider text-faint">you</span></td>
                <td class="px-4 py-3">
                  <select :value="sysOf(u)" @change="setSys(u, $event.target.value)" :title="SYS.find(r => r.v === sysOf(u))?.desc" class="rounded-lg border border-line bg-surface2 px-2 py-1 text-sm text-fg focus:border-accent/60 focus:outline-none">
                    <option v-for="r in SYS" :key="r.v" :value="r.v">{{ r.label }}</option>
                  </select>
                </td>
                <td class="px-4 py-3 text-right tabular-nums text-muted">{{ u.namespaces }}</td>
                <td class="px-4 py-3 text-right">
                  <button v-if="u.id !== auth.user?.id" @click="removeUser(u)" title="Delete user" class="text-muted hover:text-rose-400">
                    <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2m3 0v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/></svg>
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      <!-- per-namespace access -->
      <section class="space-y-3">
        <div class="flex items-center gap-3">
          <h2 class="text-sm font-semibold text-fg">Namespace access</h2>
          <select v-model="nsId" class="rounded-lg border border-line bg-surface2 px-3 py-1.5 text-sm text-fg focus:border-accent/60 focus:outline-none">
            <option v-for="n in namespaces" :key="n.id" :value="n.id">{{ n.name }}</option>
          </select>
        </div>
        <form @submit.prevent="addMember" class="flex flex-wrap items-start gap-2">
          <input v-model="nm.email" placeholder="existing user email" class="min-w-48 flex-1 rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg placeholder:text-faint focus:border-accent/60 focus:outline-none" />
          <select v-model="nm.role" class="rounded-lg border border-line bg-surface2 px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none">
            <option v-for="r in NS_ROLES" :key="r.v" :value="r.v">{{ r.label }}</option>
          </select>
          <button type="submit" class="rounded-lg border border-line bg-surface2 px-4 py-2 text-sm text-fg hover:border-accent/50">Grant access</button>
        </form>
        <p v-if="mErr" class="text-xs text-rose-400">{{ mErr }}</p>

        <div class="overflow-hidden rounded-xl border border-line bg-surface">
          <table class="w-full text-sm">
            <thead><tr class="border-b border-line text-left text-[11px] uppercase tracking-wider text-faint">
              <th class="px-4 py-3 font-medium">Member</th>
              <th class="px-4 py-3 font-medium">Access</th>
              <th class="px-4 py-3"></th>
            </tr></thead>
            <tbody>
              <tr v-if="!members.length"><td colspan="3" class="px-4 py-6 text-center text-muted">No members in this namespace yet.</td></tr>
              <tr v-for="m in members" :key="m.user_id" class="border-b border-line/60 last:border-0 hover:bg-surface2/50">
                <td class="px-4 py-3 text-fg">{{ m.email }}</td>
                <td class="px-4 py-3">
                  <select :value="m.role" @change="setMemberRole(m, $event.target.value)" class="rounded-lg border border-line bg-surface2 px-2 py-1 text-sm text-fg focus:border-accent/60 focus:outline-none">
                    <option v-for="r in NS_ROLES" :key="r.v" :value="r.v">{{ r.label }}</option>
                  </select>
                </td>
                <td class="px-4 py-3 text-right">
                  <button @click="removeMember(m)" title="Remove from namespace" class="text-muted hover:text-rose-400">
                    <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6 6 18M6 6l12 12"/></svg>
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
        <p class="text-xs text-faint"><b>Read</b> = view metrics · <b>Write</b> = add/edit systems &amp; monitors · <b>Owner</b> = also manage members. System admins see every namespace automatically.</p>
      </section>
    </div>
  </AppShell>
</template>
