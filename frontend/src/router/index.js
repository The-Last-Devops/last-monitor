import { createRouter, createWebHistory } from 'vue-router'
import { useAuth } from '../stores/auth'

// Pages are imported eagerly (not lazy `() => import()`). Lazy routes fetch a
// JS chunk on first navigation, and the router renders nothing until it lands —
// that gap is the brief blank/black flash when switching pages. Eager imports
// make every navigation synchronous, so the new page paints in one frame. The
// whole app is small enough (single-binary internal tool) that one bundle is fine.
import Login from '../pages/Login.vue'
import Systems from '../pages/Systems.vue'
import SystemDetail from '../pages/SystemDetail.vue'
import Namespaces from '../pages/Namespaces.vue'
import Members from '../pages/Members.vue'
import Monitors from '../pages/Monitors.vue'
import MonitorDetail from '../pages/MonitorDetail.vue'
import Notifications from '../pages/Notifications.vue'
import Events from '../pages/Events.vue'
import Alerts from '../pages/Alerts.vue'
import DataRetention from '../pages/DataRetention.vue'
import Backup from '../pages/Backup.vue'
import Audit from '../pages/Audit.vue'
import ApiTokens from '../pages/ApiTokens.vue'
import About from '../pages/About.vue'

const routes = [
  { path: '/login', name: 'login', component: Login, meta: { public: true } },
  { path: '/', name: 'systems', component: Systems },
  { path: '/attention', name: 'attention', component: Systems },
  { path: '/system/:id', name: 'system', component: SystemDetail },
  { path: '/namespaces', name: 'namespaces', component: Namespaces },
  { path: '/members', name: 'members', component: Members },
  { path: '/monitors', name: 'monitors', component: Monitors },
  { path: '/monitor/:id', name: 'monitor', component: MonitorDetail },
  { path: '/notifications', name: 'notifications', component: Notifications },
  { path: '/events', name: 'events', component: Events },
  { path: '/alerts', name: 'alerts', component: Alerts },
  { path: '/data', name: 'data', component: DataRetention },
  { path: '/backup', name: 'backup', component: Backup },
  { path: '/audit', name: 'audit', component: Audit },
  { path: '/tokens', name: 'tokens', component: ApiTokens },
  { path: '/about', name: 'about', component: About },
]

const router = createRouter({
  history: createWebHistory('/'),
  routes,
})

router.beforeEach(async (to, from) => {
  const auth = useAuth()
  if (!auth.ready) await auth.bootstrap()
  if (!to.meta.public && !auth.isAuthed) return { name: 'login', query: { next: to.fullPath } }
  if (to.name === 'login' && auth.isAuthed) return { name: 'systems' }
  // Keep the namespace selection in the URL across navigation — never drop ?ns
  // when moving to another page. (Same-page changes are respected, so clearing
  // the selector to "all namespaces" still works.)
  if (to.path !== from.path && to.query.ns == null && from.query.ns != null) {
    return { path: to.path, query: { ...to.query, ns: from.query.ns }, hash: to.hash }
  }
})

export default router
