<script setup>
import { ref, onMounted, onBeforeUnmount, watch, computed } from 'vue'
import uPlot from 'uplot'
import 'uplot/dist/uPlot.min.css'
import { useUi } from '../stores/ui'

// props.series: [{ name, color, data: number[] }]; props.time: unix seconds[]
const props = defineProps({
  time: { type: Array, default: () => [] },
  series: { type: Array, default: () => [] },
  unit: { type: String, default: '' },
  height: { type: Number, default: 150 },
})

const ui = useUi()
const el = ref(null)
let u = null
let ro = null

function cssVar(name) {
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  return v ? `rgb(${v})` : '#888'
}
function fmt(v) {
  if (v == null) return '—'
  if (props.unit === '%' || props.unit === '°C') return Math.round(v) + props.unit
  if (/B\/?s?/.test(props.unit)) {
    const u = ['B', 'K', 'M', 'G']; let i = 0; let n = v
    while (n >= 1024 && i < 3) { n /= 1024; i++ }
    return n.toFixed(n < 10 && i > 0 ? 1 : 0) + ' ' + u[i] + (props.unit.includes('/s') ? '/s' : '')
  }
  return v.toFixed(0) + props.unit
}

const uData = computed(() => [props.time, ...props.series.map((s) => s.data)])

function opts() {
  const axis = cssVar('--muted')
  const grid = cssVar('--line')
  return {
    width: el.value?.clientWidth || 400,
    height: props.height,
    padding: [8, 8, 0, 0],
    legend: { show: true },
    cursor: { points: { size: 7 }, focus: { prox: 30 } },
    scales: { x: { time: true } },
    series: [
      { label: 'time' },
      ...props.series.map((s) => ({
        label: s.name,
        stroke: s.color,
        width: 1.6,
        fill: s.color + '22',
        points: { show: false },
        value: (_u, v) => fmt(v),
      })),
    ],
    axes: [
      { stroke: axis, grid: { stroke: grid, width: 1 }, ticks: { stroke: grid }, font: '11px ui-monospace, monospace' },
      { stroke: axis, grid: { stroke: grid, width: 1 }, ticks: { stroke: grid }, font: '11px ui-monospace, monospace', size: 44, values: (_u, vals) => vals.map(fmt) },
    ],
  }
}

function build() {
  if (u) { u.destroy(); u = null }
  if (!el.value) return
  u = new uPlot(opts(), uData.value, el.value)
}

onMounted(() => {
  build()
  ro = new ResizeObserver(() => u && u.setSize({ width: el.value.clientWidth, height: props.height }))
  ro.observe(el.value)
})
onBeforeUnmount(() => { ro && ro.disconnect(); u && u.destroy() })

// live data updates: just push new data into the existing chart
watch(uData, (d) => { if (u) u.setData(d) })
// theme change → rebuild with new axis/grid colors
watch(() => ui.light, () => build())
</script>

<template>
  <div ref="el" class="uplot-host w-full"></div>
</template>

<style>
/* uPlot legend: blend into the dark/light theme */
.uplot, .u-legend { font-family: ui-monospace, monospace; }
.u-legend { color: rgb(var(--muted)); font-size: 11px; }
.u-legend .u-marker { width: 10px; height: 10px; }
.u-legend th { color: rgb(var(--fg)); font-weight: 500; }
.u-legend .u-value { color: rgb(var(--fg)); }
.u-tooltip { background: rgb(var(--surface2)); color: rgb(var(--fg)); }
</style>
