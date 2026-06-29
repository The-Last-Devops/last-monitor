# Two-factor auth (TOTP) & passkeys (WebAuthn) — design

Status: **TOTP core landed & unit-tested (`crates/hub/src/totp.rs`); login wiring +
passkeys are planned.** This documents the design so the login-flow changes (which
can lock people out if wrong) are reviewed before they ship, and so the passkey ↔
master-key conflict below is resolved deliberately, not by accident.

Relates to: [exec-design.md](exec-design.md) (the SSH-key master key), `auth.rs`,
`masterkey.rs`.

---

## 1. TOTP 2FA (RFC 6238)

A second factor on top of the existing password login. **Opt-in per user** — an
account that hasn't enrolled logs in exactly as today, so a bug in the 2FA path can
never lock out users who haven't turned it on.

The maths is done and proven against the RFC 6238 / RFC 4648 test vectors in
`totp.rs` (`base32_encode/decode`, `verify`, `provisioning_uri`). No new heavy deps —
`hmac` + `sha1` were already in the tree (via russh).

### Schema (migration 0022, planned)

```sql
ALTER TABLE users
  ADD COLUMN totp_secret_enc   BYTEA,   -- TOTP secret, sealed under the app secret
  ADD COLUMN totp_kid          TEXT,    -- app-secret id that sealed it (NULL = raw, dev)
  ADD COLUMN totp_enabled      BOOLEAN NOT NULL DEFAULT false,
  ADD COLUMN totp_backup_codes TEXT;    -- JSON array of sha256(code) hashes, one-time use
```

The TOTP secret is sealed with the **same app secret** that wraps master keys
(`exec_crypto::AppKey`), so a DB-only leak can't mint codes. It rotates with the
app secret (extend `rotate-app-secret` to re-wrap `totp_secret_enc` too).

### Endpoints (planned, all session-authed = the signed-in user)

- `POST /api/me/2fa/start` → generate a random 20-byte secret (not yet enabled),
  store it sealed with `totp_enabled=false`, return `{ secret_base32, otpauth_uri }`.
- `POST /api/me/2fa/enable { code }` → `totp::verify` the code against the pending
  secret; on success set `totp_enabled=true`, generate 10 backup codes, store their
  hashes, and return the codes **once** (shown to the user to save).
- `POST /api/me/2fa/disable { password }` → verify password, clear all totp columns.
- `GET /api/me/2fa` → `{ enabled, backup_codes_remaining }` for the account UI.

### Login flow change (the sensitive part — review before shipping)

`login` today returns the user + sets the session cookie. New contract:

1. Verify email + password as today.
2. If `totp_enabled` **and** `req.totp_code` is absent → return `200 { twofa_required: true }`
   and **do not** mint a session. The SPA shows a 6-digit code field and re-POSTs with it.
3. If `totp_enabled` and a code is present → accept a valid TOTP code (`totp::verify`)
   **or** an unused backup code (consume it). Otherwise `401`.
4. Mint the session only after the factor passes.

> **Escape hatch:** an admin who loses their authenticator clears the columns via DB
> (`UPDATE users SET totp_enabled=false, totp_secret_enc=NULL WHERE email=…`) or via
> the admin user PATCH (extend it to reset 2FA). Document this so 2FA can't brick an org.

### Frontend (planned)

- Account menu → "Two-factor auth": start → show `otpauth_uri` as a QR (render inline,
  no new npm dep — or show the `secret_base32` for manual entry) → confirm with a code →
  display backup codes once → done. Disable with password.
- `Login.vue`: when the API returns `twofa_required`, swap to a code step and re-submit.

---

## 2. Passkeys / WebAuthn — and the master-key conflict

Passkeys are **phishing-resistant** and a great fit, but two things must be decided first.

### ⚠️ Conflict: passkey login has no password, but the SSH-key master key needs one

Today a user's SSH-key **master key** is unwrapped by a KEK derived from their
**password** (`masterkey.rs`). If a user authenticates with a passkey, there is **no
password** in the request — so the hub cannot unwrap their master key, and their stored
SSH keys become unusable for that session. This is fundamental, not a detail. Options:

1. **Password stays the key-custody factor; passkey is only a login factor.** Simplest
   and safest: passkeys (and/or TOTP) gate *login*, but to *open an SSH console* the user
   still types their account password at step-up (as they do now). Master-key design is
   untouched. **Recommended.**
2. **WebAuthn PRF extension** derives a stable secret from the passkey; wrap a *second*
   copy of the master key under a PRF-derived KEK. Powerful (passwordless end-to-end) but
   PRF support is uneven across authenticators, and it materially complicates key custody.
3. **A separate "exec unlock" passphrase** decoupled from the login password. More moving
   parts; only worth it if passwordless login is a hard requirement.

Pick (1) for the first pass; revisit (2) if true passwordless is wanted later.

### Dependency / "stay lightweight" note

WebAuthn is **not** in the dependency tree today. A correct implementation needs a
vetted library (e.g. `webauthn-rs`), which is a sizeable tree — in tension with guiding
principle #2 (stay lightweight). Hand-rolling WebAuthn is **not** advisable (attestation,
COSE keys, signature counters, origin/RP-ID checks are easy to get subtly wrong). So this
is a deliberate "add one significant, well-audited dep" decision for the operator to OK.

### Schema sketch (planned)

```sql
CREATE TABLE webauthn_credentials (
  id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  cred_id       BYTEA NOT NULL UNIQUE,   -- credential id
  public_key    BYTEA NOT NULL,          -- COSE public key
  sign_count    BIGINT NOT NULL DEFAULT 0,
  name          TEXT NOT NULL,           -- "MacBook Touch ID", "YubiKey 5"
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_used     TIMESTAMPTZ
);
```

Register/authenticate ceremonies live in `auth.rs` (where sessions are minted) so the
three auth paths (cookie / PAT / agent token) stay in one place; the SPA uses the
`navigator.credentials` API. `RP_ID` / `origin` come from config (must match the served
domain) — getting these wrong silently breaks passkeys, so they need explicit settings.

---

## Recommended phasing

1. **TOTP wiring** (low risk, no new deps): migration 0022 + endpoints + opt-in login
   step + account UI. Ship behind the per-user opt-in with the DB escape hatch documented.
2. **Passkeys as a login factor** (option 1 above): add `webauthn-rs`, registration +
   login ceremonies, keep password as the exec-unlock factor. One reviewed dependency.
3. *(Optional, later)* passwordless via the PRF extension (option 2) if desired.
