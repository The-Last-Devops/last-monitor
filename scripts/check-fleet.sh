#!/usr/bin/env bash
# Verify the fleet overlay endpoint + CPU breakdown columns end-to-end:
# login -> GET /api/fleet (per-host series) and a node's /metrics (breakdown).
set -u
HUB_URL="${HUB_URL:-http://localhost:8080}"
ADMIN_EMAIL="${ADMIN_EMAIL:-admin@local}"
ADMIN_PASSWORD="${ADMIN_PASSWORD:-admin123}"
RANGE="${RANGE:-1h}"

node - "$HUB_URL" "$ADMIN_EMAIL" "$ADMIN_PASSWORD" "$RANGE" <<'EOF'
const [HUB, email, password, range] = process.argv.slice(2);
let cookie = '';
async function api(m, p, b) {
  const h = {}; if (cookie) h.cookie = cookie; if (b) h['content-type'] = 'application/json';
  const r = await fetch(HUB + p, { method: m, headers: h, body: b && JSON.stringify(b) });
  const sc = r.headers.get('set-cookie'); if (sc) cookie = sc.split(';')[0];
  if (!r.ok) throw new Error(`${m} ${p} -> ${r.status}`);
  return r.headers.get('content-type')?.includes('json') ? r.json() : r.text();
}
const last = (d) => { if (!d) return null; for (let i = d.length - 1; i >= 0; i--) if (d[i] != null) return d[i]; return null; };
(async () => {
  await api('POST', '/api/auth/login', { email, password });

  const f = await api('GET', `/api/fleet?range=${range}`);
  console.log(`fleet[${range}]: ${f.t.length} buckets  cpu=${f.cpu.length} mem=${f.mem.length} disk=${f.disk.length} net=${f.net.length} hosts`);
  console.log('  cpu sample:', f.cpu.slice(0, 3).map(s => `${s.name}=${Math.round(last(s.data) ?? 0)}%`).join('  '));

  // check CPU breakdown: scan systems for one that reports it (Linux /proc/stat;
  // macOS hosts legitimately report none).
  const systems = await api('GET', '/api/systems');
  for (const s of systems.slice(0, 8)) {
    const m = await api('GET', `/api/systems/${s.id}/metrics?range=${range}`);
    const sum = (k) => (last(m[k]) ?? 0).toFixed(1);
    const has = m.cpu_user && m.cpu_user.some(v => v > 0);
    console.log(`breakdown[${s.name}]: present=${has}  user=${sum('cpu_user')} system=${sum('cpu_system')} iowait=${sum('cpu_iowait')} steal=${sum('cpu_steal')}  load1=${sum('load1')}`);
  }
})().catch(e => { console.error(e.message); process.exit(1); });
EOF
