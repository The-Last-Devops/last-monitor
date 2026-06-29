import { defineStore } from 'pinia'
import { api } from '../lib/api'
import { dropCache } from '../lib/cache'

export const useAuth = defineStore('auth', {
  state: () => ({
    user: null,        // { id, email, is_admin }
    ready: false,      // true once session checked
    needsSetup: false, // true when no users exist → first-run wizard
  }),
  getters: {
    isAuthed: (s) => !!s.user,
  },
  actions: {
    async bootstrap() {
      // First-run check, then resolve current session.
      try {
        const s = await api.get('/api/setup')
        this.needsSetup = !!s.needs_setup
      } catch { this.needsSetup = false }
      try {
        this.user = await api.get('/api/me')
      } catch { this.user = null }
      this.ready = true
    },
    async login(email, password, totpCode) {
      const r = await api.post('/api/auth/login', { email, password, totp_code: totpCode })
      if (r && r.twofa_required) return { twofaRequired: true } // need a 2FA code; no session yet
      this.user = r
      this.needsSetup = false
      return { twofaRequired: false }
    },
    async createAdmin(email, password) {
      this.user = await api.post('/api/setup', { email, password })
      this.needsSetup = false
    },
    async logout() {
      await api.post('/api/auth/logout')
      this.user = null
      dropCache() // never paint one user's cached data for the next
    },
  },
})
