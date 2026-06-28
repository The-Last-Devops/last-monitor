<script setup>
// Shell settings for a host: shell enable/port (owner-only), the caller's own SSH
// credential (set / view / delete), and an "Open console" launcher. Parent passes
// the system id + display name; this owns its own fetch of GET .../shell so it can
// refresh after a mutation without touching the parent's metric polling.
import { ref, computed, onMounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import { api } from '../lib/api'
import { confirm } from '../lib/confirm'

const props = defineProps({
  id: { required: true },
  name: { type: String, default: '' },
})

const router = useRouter()

// shell state from the API:
//   { shell_enabled, ssh_port, tunnel_online, can_exec, credential: null | { ssh_user, key_fingerprint } }
const shell = ref(null)
const loaded = ref(false)
const loadErr = ref('')

async function load() {
  try {
    shell.value = await api.get(`/api/systems/${props.id}/shell`)
    loadErr.value = ''
  } catch (e) {
    if (e.status === 403) { shell.value = { can_exec: false } } // treated as "no access" below
    else loadErr.value = 'Failed to load shell settings.'
  } finally {
    loaded.value = true
  }
}
onMounted(load)

const canExec = computed(() => !!shell.value?.can_exec)
const credential = computed(() => shell.value?.credential || null)
const canOpenConsole = computed(
  () => canExec.value && shell.value?.shell_enabled && !!credential.value && shell.value?.tunnel_online,
)

// ---- owner-only: enable/disable shell + ssh port ----
const portInput = ref(22)
const savingShell = ref(false)
const shellMsg = ref('')
function syncPort() { portInput.value = shell.value?.ssh_port || 22 }

async function saveShell(enabled) {
  shellMsg.value = ''
  const port = Number(portInput.value)
  if (!Number.isInteger(port) || port < 1 || port > 65535) { shellMsg.value = 'Port must be 1–65535.'; return }
  savingShell.value = true
  try {
    await api.put(`/api/systems/${props.id}/shell`, { shell_enabled: enabled, ssh_port: port })
    await load(); syncPort()
  } catch (e) {
    shellMsg.value = e.status === 403 ? 'Only the namespace owner can change this.' : `Failed (${e.status}).`
  } finally {
    savingShell.value = false
  }
}

// ---- the caller's own SSH credential ----
const cred = ref({ ssh_user: '', private_key: '', password: '' })
const savingCred = ref(false)
const credMsg = ref('')

async function saveCred() {
  credMsg.value = ''
  if (!cred.value.ssh_user.trim()) { credMsg.value = 'SSH user is required.'; return }
  if (!cred.value.private_key.trim()) { credMsg.value = 'Paste your private key.'; return }
  if (!cred.value.password) { credMsg.value = 'Enter your account password to encrypt the key.'; return }
  savingCred.value = true
  try {
    await api.put(`/api/systems/${props.id}/ssh-cred`, {
      ssh_user: cred.value.ssh_user.trim(),
      private_key: cred.value.private_key,
      password: cred.value.password,
    })
    cred.value = { ssh_user: '', private_key: '', password: '' }
    await load()
  } catch (e) {
    credMsg.value =
      e.status === 400 ? 'Invalid key, inputs, or wrong password.'
      : e.status === 403 ? "You don't have shell access on this host."
      : `Failed (${e.status}).`
  } finally {
    savingCred.value = false
  }
}

async function deleteCred() {
  if (!(await confirm({ title: 'Delete SSH credential?', message: 'Your stored key for this host is removed. You can add it again later.', danger: true, confirmText: 'Delete' }))) return
  try { await api.del(`/api/systems/${props.id}/ssh-cred`); await load() }
  catch (e) { credMsg.value = `Failed (${e.status}).` }
}

function openConsole() {
  router.push({ name: 'console', params: { id: props.id }, query: { name: props.name } })
}

// keep the port input in sync once data lands
watch(shell, syncPort)
</script>

<template>
  <div class="mb-4 rounded-xl border border-line bg-surface p-4">
    <div class="mb-3 flex items-center gap-2">
      <span class="text-[11px] font-semibold uppercase tracking-wider text-faint">Shell</span>
      <span v-if="loaded && shell?.shell_enabled" class="rounded-full bg-surface2 px-2 py-0.5 text-[10px] text-accent">enabled</span>
      <span v-else-if="loaded && canExec" class="rounded-full bg-surface2 px-2 py-0.5 text-[10px] text-faint">disabled</span>
      <button v-if="canOpenConsole" @click="openConsole"
        class="ml-auto inline-flex items-center gap-1.5 rounded-lg bg-accent px-3 py-1.5 text-xs font-semibold text-accentfg hover:opacity-90">
        <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m4 17 6-6-6-6M12 19h8"/></svg>
        Open console
      </button>
    </div>

    <p v-if="!loaded" class="text-xs text-faint">Loading…</p>
    <p v-else-if="loadErr" class="text-xs text-rose-400">{{ loadErr }}</p>

    <!-- no exec permission -->
    <p v-else-if="!canExec" class="text-xs text-faint">You don't have shell access on this host.</p>

    <template v-else>
      <!-- status line -->
      <div class="mb-3 flex flex-wrap items-center gap-x-5 gap-y-1.5 text-xs">
        <span><span class="text-faint">SSH port</span> <span class="text-fg tabular-nums">{{ shell.ssh_port }}</span></span>
        <span class="flex items-center gap-1.5">
          <span class="h-1.5 w-1.5 rounded-full" :class="shell.tunnel_online ? 'bg-accent' : 'bg-faint'"></span>
          <span :class="shell.tunnel_online ? 'text-fg' : 'text-faint'">{{ shell.tunnel_online ? 'Agent online' : 'Agent offline' }}</span>
        </span>
      </div>

      <!-- owner-only enable/port form. We show it to anyone with exec and let a 403 surface
           a message, since the API doesn't return an explicit owner flag. -->
      <div class="mb-4 flex flex-wrap items-end gap-3 rounded-lg border border-line bg-surface2 p-3">
        <label class="block">
          <span class="mb-1 block text-[11px] font-semibold uppercase tracking-wide text-faint">SSH port</span>
          <input v-model.number="portInput" type="number" min="1" max="65535"
            class="w-28 rounded-lg border border-line bg-bg px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none" />
        </label>
        <div class="flex gap-2">
          <button v-if="!shell.shell_enabled" :disabled="savingShell" @click="saveShell(true)"
            class="rounded-lg bg-accent px-3 py-2 text-sm font-semibold text-accentfg hover:opacity-90 disabled:opacity-50">{{ savingShell ? 'Saving…' : 'Enable shell' }}</button>
          <template v-else>
            <button :disabled="savingShell" @click="saveShell(true)"
              class="rounded-lg border border-line bg-bg px-3 py-2 text-sm text-fg hover:border-accent/50 disabled:opacity-50">{{ savingShell ? 'Saving…' : 'Save port' }}</button>
            <button :disabled="savingShell" @click="saveShell(false)"
              class="rounded-lg border border-line bg-bg px-3 py-2 text-sm text-muted hover:text-rose-400 hover:border-rose-400/50 disabled:opacity-50">Disable</button>
          </template>
        </div>
        <p v-if="shellMsg" class="w-full text-xs text-rose-400">{{ shellMsg }}</p>
        <p class="w-full text-[11px] text-faint">Owner-only. Enables interactive SSH console access for this host.</p>
      </div>

      <!-- the caller's own SSH credential -->
      <div v-if="credential" class="rounded-lg border border-line bg-surface2 p-3">
        <div class="mb-1 text-[11px] font-semibold uppercase tracking-wide text-faint">Your SSH credential</div>
        <div class="flex flex-wrap items-center gap-x-5 gap-y-1.5 text-xs">
          <span><span class="text-faint">User</span> <span class="text-fg">{{ credential.ssh_user }}</span></span>
          <span class="min-w-0"><span class="text-faint">Key</span> <span class="font-mono text-muted break-all">{{ credential.key_fingerprint }}</span></span>
          <button @click="deleteCred" class="ml-auto rounded-lg p-1.5 text-muted hover:bg-bg hover:text-rose-400" v-tip="'Delete credential'">
            <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2m3 0v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/></svg>
          </button>
        </div>
      </div>

      <form v-else @submit.prevent="saveCred" class="space-y-3 rounded-lg border border-line bg-surface2 p-3">
        <div class="text-[11px] font-semibold uppercase tracking-wide text-faint">Add your SSH credential</div>
        <label class="block">
          <span class="mb-1 block text-[11px] text-faint">SSH user</span>
          <input v-model="cred.ssh_user" placeholder="e.g. ubuntu, root"
            class="w-full rounded-lg border border-line bg-bg px-3 py-2 text-sm text-fg placeholder:text-faint focus:border-accent/60 focus:outline-none" />
        </label>
        <label class="block">
          <span class="mb-1 block text-[11px] text-faint">Private key</span>
          <textarea v-model="cred.private_key" rows="5" spellcheck="false"
            placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
            class="w-full rounded-lg border border-line bg-bg px-3 py-2 font-mono text-xs text-fg placeholder:text-faint focus:border-accent/60 focus:outline-none"></textarea>
        </label>
        <label class="block">
          <span class="mb-1 block text-[11px] text-faint">Your account password</span>
          <input v-model="cred.password" type="password" autocomplete="current-password"
            class="w-full rounded-lg border border-line bg-bg px-3 py-2 text-sm text-fg focus:border-accent/60 focus:outline-none" />
        </label>
        <p v-if="credMsg" class="text-xs text-rose-400">{{ credMsg }}</p>
        <div class="flex items-center gap-3">
          <button type="submit" :disabled="savingCred"
            class="rounded-lg bg-accent px-3 py-2 text-sm font-semibold text-accentfg hover:opacity-90 disabled:opacity-50">{{ savingCred ? 'Saving…' : 'Save credential' }}</button>
          <p class="text-[11px] text-faint">Your key is encrypted with your password; we can't read it without you.</p>
        </div>
      </form>
    </template>
  </div>
</template>
