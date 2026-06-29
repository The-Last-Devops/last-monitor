// Detect when a newer build has been deployed while this tab stayed open, so a
// long-lived SPA tab never silently runs stale code. We don't fight the cache —
// the hub already serves index.html `no-cache` and hashed assets `immutable`; we
// just compare the JS bundle hash this tab loaded with the one the *current*
// index.html references, and surface a "Reload" banner on a mismatch.
import { ref } from 'vue'

export const updateAvailable = ref(false)

const ASSET_RE = /assets\/index-[\w-]+\.js/

// The hashed JS bundle THIS tab is running (from its own module <script src>).
// In dev (Vite) there's no hashed bundle → returns null and the watcher no-ops.
function runningAsset() {
  for (const s of document.querySelectorAll('script[type="module"]')) {
    const m = s.src.match(ASSET_RE)
    if (m) return m[0]
  }
  return null
}

// The bundle the freshly-fetched index.html references (bypassing the HTTP cache).
async function deployedAsset() {
  try {
    const html = await fetch('/', { cache: 'no-store' }).then((r) => r.text())
    const m = html.match(ASSET_RE)
    return m ? m[0] : null
  } catch {
    return null
  }
}

let timer
export function startBuildWatch() {
  const running = runningAsset()
  if (!running) return // dev / no hashed bundle — nothing to compare
  const check = async () => {
    if (updateAvailable.value) return
    const deployed = await deployedAsset()
    if (deployed && deployed !== running) updateAvailable.value = true
  }
  timer = setInterval(check, 5 * 60 * 1000) // every 5 min
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') check() // and whenever the tab is refocused
  })
  setTimeout(check, 15000) // and once shortly after load
}
