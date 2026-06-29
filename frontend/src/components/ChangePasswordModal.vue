<script setup>
// Self-service "change your own password" dialog. Posts to /api/me/password, which
// re-wraps the user's SSH-key master key old→new so their keys keep working (unlike
// an admin reset). Validates the new password client-side to mirror the server.
import { ref, watch } from 'vue'
import { api } from '../lib/api'

const props = defineProps({ open: { type: Boolean, default: false } })
const emit = defineEmits(['close'])

const current = ref('')
const next = ref('')
const confirm = ref('')
const saving = ref(false)
const err = ref('')
const done = ref(false)

// Mirror the server policy (api::valid_password): ≥12 chars, mixed case + digit.
function policyMsg(p) {
  if (p.length < 12) return 'At least 12 characters.'
  if (!/[a-z]/.test(p) || !/[A-Z]/.test(p) || !/[0-9]/.test(p)) return 'Mix upper, lower, and a digit.'
  return ''
}

function reset() { current.value = ''; next.value = ''; confirm.value = ''; err.value = ''; done.value = false; saving.value = false }
watch(() => props.open, (o) => { if (o) reset() })

async function submit() {
  err.value = ''
  const pm = policyMsg(next.value)
  if (pm) { err.value = pm; return }
  if (next.value !== confirm.value) { err.value = 'New passwords do not match.'; return }
  saving.value = true
  try {
    await api.post('/api/me/password', { current_password: current.value, new_password: next.value })
    done.value = true
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
  <div v-if="open" class="fixed inset-0 z-50 flex items-center justify-center p-4">
    <div class="absolute inset-0 bg-black/60" @click="emit('close')"></div>
    <div class="relative w-full max-w-sm rounded-xl border border-line bg-surface p-6 shadow-xl">
      <div class="mb-4 flex items-center gap-2">
        <VIcon name="lock" :size="18" class="text-accent" />
        <h2 class="text-h2 font-semibold text-fg">Change password</h2>
      </div>

      <!-- success -->
      <div v-if="done" class="space-y-4">
        <p class="text-sm text-muted">Your password has been changed. Your SSH keys still work.</p>
        <div class="flex justify-end">
          <button @click="emit('close')" class="rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-accentfg hover:opacity-90">Done</button>
        </div>
      </div>

      <form v-else @submit.prevent="submit" autocomplete="off" class="space-y-3">
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
        <div class="flex justify-end gap-2.5 pt-1">
          <button type="button" @click="emit('close')" class="rounded-lg px-3 py-2 text-sm text-muted hover:text-fg">Cancel</button>
          <button type="submit" :disabled="saving" class="rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-accentfg hover:opacity-90 disabled:opacity-50">{{ saving ? 'Saving…' : 'Change password' }}</button>
        </div>
      </form>
    </div>
  </div>
</template>
