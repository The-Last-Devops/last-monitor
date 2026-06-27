import { defineStore } from 'pinia'
import { api } from '../lib/api'

const REPO = 'The-Last-Devops/vantage'

// naive semver compare (a vs b) → >0 if a newer
function cmp(a, b) {
  const pa = a.split('.').map(Number), pb = b.split('.').map(Number)
  for (let i = 0; i < 3; i++) { if ((pa[i] || 0) !== (pb[i] || 0)) return (pa[i] || 0) - (pb[i] || 0) }
  return 0
}

// Current build version + latest GitHub release, fetched once and shared by the
// shell's version badge and the About page. Being a singleton store, it survives
// the per-navigation remount of AppShell so GitHub is hit at most once per session.
export const useVersion = defineStore('version', {
  state: () => ({
    current: '',     // running build version, e.g. "1.4.0"
    latestTag: '',   // newest release tag, e.g. "v1.4.1" ('' = unknown/unreachable)
    loaded: false,   // /api/about resolved
    checked: false,  // GitHub release check finished (ok or failed)
  }),
  getters: {
    // Only claim "outdated" when we actually know the latest tag and it's newer.
    isOutdated: (s) => {
      if (!s.current || !s.latestTag) return false
      const tag = s.latestTag.replace(/^v/, '')
      return tag !== s.current && cmp(tag, s.current) > 0
    },
  },
  actions: {
    async ensureLoaded() {
      if (this.loaded) return
      this.loaded = true
      try { const a = await api.get('/api/about'); this.current = a?.version || '' } catch {}
      this.checkLatest()
    },
    async checkLatest() {
      if (this.checked) return
      try {
        const r = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
          headers: { Accept: 'application/vnd.github+json' },
        })
        if (r.ok) { const j = await r.json(); this.latestTag = j.tag_name || '' }
      } catch { /* offline / rate-limited → stay neutral, no false "outdated" */ }
      finally { this.checked = true }
    },
  },
})
