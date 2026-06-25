// Resolve `promise` no sooner than `ms`, so a very fast first load still shows
// the loader long enough to read as intentional rather than a flicker/error.
// Use only on the *first* load — never on background polling, or you add latency
// to every refresh for no reason.
// Minimum loader display time so a fast first load reads as intentional rather
// than a flicker/error.
export function minLoad(promise, ms = 200) {
  const start = Date.now()
  return Promise.resolve(promise).then(async (value) => {
    const rest = ms - (Date.now() - start)
    if (rest > 0) await new Promise((r) => setTimeout(r, rest))
    return value
  })
}
