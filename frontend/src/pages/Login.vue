<script setup>
import { ref, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useAuth } from '../stores/auth'
import { passwordProblem } from '../lib/password'

const auth = useAuth()
const route = useRoute()
const router = useRouter()

const email = ref('')
const password = ref('')
const confirm = ref('')
const error = ref('')
const busy = ref(false)
const twofa = ref(false)   // second step: the account has 2FA, collect a code
const totpCode = ref('')

const setup = () => auth.needsSetup

// Live policy hint while creating the admin (empty once the password is acceptable).
const pwHint = computed(() => (setup() && password.value ? passwordProblem(password.value) : ''))

async function submit() {
  error.value = ''
  if (setup()) {
    const problem = passwordProblem(password.value)
    if (problem) { error.value = problem; return }
    if (password.value !== confirm.value) { error.value = 'Passwords do not match'; return }
  }
  busy.value = true
  try {
    if (setup()) {
      await auth.createAdmin(email.value, password.value)
    } else {
      const res = await auth.login(email.value, password.value, twofa.value ? totpCode.value : undefined)
      if (res.twofaRequired) { twofa.value = true; busy.value = false; return } // show the code step
    }
    router.push(route.query.next || { name: 'systems' })
  } catch (e) {
    error.value = setup()
      ? 'Could not create admin (maybe one already exists)'
      : twofa.value ? 'Invalid or expired code'
      : e.status === 401 ? 'Invalid email or password' : 'Login failed'
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="flex min-h-screen items-center justify-center px-6">
    <div class="w-full max-w-sm">
      <div class="mb-8 flex items-center justify-center gap-2.5">
        <span class="vantage-logo inline-block h-7 w-7 rounded-md"></span>
        <span class="text-xl font-semibold tracking-tight text-fg">Vantage</span>
      </div>

      <form class="space-y-4 rounded-xl border border-line bg-surface p-7 shadow-2xl" @submit.prevent="submit">
        <div>
          <h1 class="text-base font-semibold text-fg">{{ setup() ? 'Create admin account' : twofa ? 'Two-factor code' : 'Sign in' }}</h1>
          <p class="mt-1 text-sm text-muted">
            {{ setup() ? 'First run — set up the administrator account.' : twofa ? 'Enter the 6-digit code from your authenticator app.' : 'Monitor your fleet & services.' }}
          </p>
        </div>

        <!-- step 2: 2FA code -->
        <template v-if="twofa">
          <label class="block text-sm">
            <span class="text-muted">Authentication code</span>
            <input v-model="totpCode" inputmode="numeric" autocomplete="one-time-code" autofocus placeholder="123456"
              class="mt-1.5 w-full rounded-lg border border-line bg-surface2 px-3 py-2.5 text-center font-mono text-lg tracking-[0.3em] text-fg outline-none transition focus:border-accent" />
            <span class="mt-1 block text-xs text-faint">Or enter one of your backup codes.</span>
          </label>
        </template>

        <!-- step 1: credentials -->
        <template v-else>
          <label class="block text-sm">
            <span class="text-muted">Email</span>
            <input v-model="email" type="email" required autocomplete="username"
              class="mt-1.5 w-full rounded-lg border border-line bg-surface2 px-3 py-2.5 text-fg outline-none transition focus:border-accent" />
          </label>

          <label class="block text-sm">
            <span class="text-muted">Password</span>
            <input v-model="password" type="password" required :autocomplete="setup() ? 'new-password' : 'current-password'"
              class="mt-1.5 w-full rounded-lg border border-line bg-surface2 px-3 py-2.5 text-fg outline-none transition focus:border-accent" />
            <span v-if="pwHint" class="mt-1 block text-xs text-warn">{{ pwHint }}</span>
            <span v-else-if="setup()" class="mt-1 block text-xs text-faint">12+ chars, mix of cases, digits &amp; symbols.</span>
          </label>

          <label v-if="setup()" class="block text-sm">
            <span class="text-muted">Confirm password</span>
            <input v-model="confirm" type="password" required autocomplete="new-password"
              class="mt-1.5 w-full rounded-lg border border-line bg-surface2 px-3 py-2.5 text-fg outline-none transition focus:border-accent" />
          </label>
        </template>

        <p v-if="error" class="text-sm text-red-500">{{ error }}</p>

        <button type="submit" :disabled="busy"
          class="w-full rounded-lg bg-accent px-4 py-2.5 font-semibold text-accentfg transition hover:opacity-90 disabled:opacity-50">
          {{ busy ? 'Working…' : setup() ? 'Create account' : twofa ? 'Verify' : 'Sign in' }}
        </button>
        <button v-if="twofa" type="button" @click="twofa = false; totpCode = ''; error = ''"
          class="w-full text-center text-xs text-muted hover:text-fg">← Back</button>

        <p v-if="!setup()" class="text-center text-xs text-faint">No public registration — accounts are provisioned by an admin.</p>
      </form>
    </div>
  </div>
</template>
