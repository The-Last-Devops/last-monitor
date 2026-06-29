<script setup>
// Account security settings: change password (re-wraps the SSH-key master key so
// keys survive — see masterkey.rs), two-factor auth (coming soon), and a pointer to
// the SSH key library. A proper page rather than a cramped dropdown modal.
import { ref } from 'vue'
import AppShell from '../components/AppShell.vue'
import { api } from '../lib/api'

// ---- change password ----
const current = ref('')
const next = ref('')
const confirm = ref('')
const saving = ref(false)
const err = ref('')
const ok = ref(false)

function policyMsg(p) {
  if (p.length < 12) return 'At least 12 characters.'
  if (!/[a-z]/.test(p) || !/[A-Z]/.test(p) || !/[0-9]/.test(p)) return 'Mix upper, lower, and a digit.'
  return ''
}

async function changePassword() {
  err.value = ''; ok.value = false
  const pm = policyMsg(next.value)
  if (pm) { err.value = pm; return }
  if (next.value !== confirm.value) { err.value = 'New passwords do not match.'; return }
  saving.value = true
  try {
    await api.post('/api/me/password', { current_password: current.value, new_password: next.value })
    ok.value = true
    current.value = ''; next.value = ''; confirm.value = ''
  } catch (e) {
    err.value = e.status === 401 ? 'Current password is wrong.'
      : e.status === 400 ? 'New password does not meet the policy.'
      : `Failed (${e.status || 'error'}).`
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <AppShell title="Security">
    <div class="space-y-4">
      <!-- Password -->
      <section class="rounded-xl border border-line bg-surface p-5">
        <div class="mb-1 flex items-center gap-2">
          <VIcon name="shield" :size="16" class="text-accent" />
          <h2 class="text-h2 font-semibold text-fg">Password</h2>
        </div>
        <p class="mb-4 text-xs text-muted">
          Changing your password keeps your SSH keys working — they're re-secured under your new password automatically.
        </p>
        <form @submit.prevent="changePassword" autocomplete="off" class="max-w-sm space-y-3">
          <label class="block">
            <span class="mb-1 block text-[11px] font-semibold uppercase tracking-wide text-faint">Current password</span>
            <input v-model="current" type="password" autocomplete="current-password" required
              class="w-full rounded-lg border border-line bg-surface2 px-3 py-2.5 text-sm text-fg focus:border-accent/60 focus:outline-none" />
          </label>
          <label class="block">
            <span class="mb-1 block text-[11px] font-semibold uppercase tracking-wide text-faint">New password</span>
            <input v-model="next" type="password" autocomplete="new-password" required
              class="w-full rounded-lg border border-line bg-surface2 px-3 py-2.5 text-sm text-fg focus:border-accent/60 focus:outline-none" />
          </label>
          <label class="block">
            <span class="mb-1 block text-[11px] font-semibold uppercase tracking-wide text-faint">Confirm new password</span>
            <input v-model="confirm" type="password" autocomplete="new-password" required
              class="w-full rounded-lg border border-line bg-surface2 px-3 py-2.5 text-sm text-fg focus:border-accent/60 focus:outline-none" />
          </label>
          <p class="text-[11px] text-faint">At least 12 characters with upper, lower, and a digit.</p>
          <p v-if="err" class="text-xs text-rose-400">{{ err }}</p>
          <p v-if="ok" class="text-xs text-ok">Password changed.</p>
          <button type="submit" :disabled="saving"
            class="rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-accentfg hover:opacity-90 disabled:opacity-50">{{ saving ? 'Saving…' : 'Change password' }}</button>
        </form>
      </section>

      <!-- Two-factor (coming soon) -->
      <section class="rounded-xl border border-line bg-surface p-5">
        <div class="mb-1 flex items-center gap-2">
          <VIcon name="lock" :size="16" class="text-muted" />
          <h2 class="text-h2 font-semibold text-fg">Two-factor authentication</h2>
          <span class="rounded-full bg-surface2 px-2 py-0.5 text-[10px] uppercase tracking-wide text-faint">Coming soon</span>
        </div>
        <p class="mb-4 max-w-prose text-xs text-muted">
          Add a second factor (an authenticator app / TOTP, then passkeys) on top of your password.
          The verification core is built; the enrollment flow is rolling out next.
        </p>
        <button disabled class="cursor-not-allowed rounded-lg border border-line px-4 py-2 text-sm text-faint">Set up authenticator</button>
      </section>

      <!-- SSH keys -->
      <section class="rounded-xl border border-line bg-surface p-5">
        <div class="mb-1 flex items-center gap-2">
          <VIcon name="ssh" :size="16" class="text-accent" />
          <h2 class="text-h2 font-semibold text-fg">SSH keys</h2>
        </div>
        <p class="mb-4 max-w-prose text-xs text-muted">
          Manage the private keys you use to open host consoles. Keys are sealed under your password and never shown again after upload.
        </p>
        <RouterLink :to="{ name: 'ssh-keys' }"
          class="inline-flex items-center gap-2 rounded-lg border border-line px-4 py-2 text-sm text-fg hover:border-accent/50">
          <VIcon name="ssh" :size="15" /> Manage SSH keys
        </RouterLink>
      </section>
    </div>
  </AppShell>
</template>
