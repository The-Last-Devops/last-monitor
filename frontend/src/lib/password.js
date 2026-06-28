// Password policy — MUST mirror crate::api::valid_password (the API is the source of
// truth; this is for instant feedback). Rules: 12–128 chars, at least 3 of {lowercase,
// uppercase, digit, symbol}, and not an obviously weak/common password.
const WEAK = [
  'password', 'passw0rd', 'qwerty', '123456', '111111', '000000', 'letmein',
  'welcome', 'iloveyou', 'abc123', 'admin', 'vantage', 'monitor',
]

/// Returns a human-readable reason the password is unacceptable, or '' if it's fine.
export function passwordProblem(s) {
  const len = [...s].length
  if (len < 12) return 'Use at least 12 characters'
  if (len > 128) return 'Use at most 128 characters'
  const classes = [
    /[a-z]/.test(s),
    /[A-Z]/.test(s),
    /[0-9]/.test(s),
    /[^a-zA-Z0-9\s]/.test(s),
  ].filter(Boolean).length
  if (classes < 3) return 'Mix at least 3 of: lowercase, uppercase, digit, symbol'
  const low = s.toLowerCase()
  if (WEAK.some((w) => low.includes(w))) return 'Too common or predictable — choose something unique'
  return ''
}

export const passwordOk = (s) => passwordProblem(s) === ''
