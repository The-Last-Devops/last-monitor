import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import router from './router'
import { useUi } from './stores/ui'
import { tip } from './lib/tooltip'
import UiSelect from './components/UiSelect.vue'
import DataTable from './components/DataTable.vue'
import StatePill from './components/StatePill.vue'
import './style.css'

const app = createApp(App)
app.use(createPinia())
useUi().applyTheme() // apply saved theme before first paint
app.directive('tip', tip) // v-tip="'text'" — themed tooltip (replaces native title=)
app.component('UiSelect', UiSelect) // themed dropdown (replaces native <select>)
app.component('DataTable', DataTable) // Rancher-style sortable/selectable table
app.component('StatePill', StatePill) // status pill (ok/warn/down/info/muted)
app.use(router)
app.mount('#app')
