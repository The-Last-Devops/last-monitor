<script setup>
// A themed replacement for the native <select>. The menu is teleported to <body>
// (so it's never clipped by a card's overflow), supports keyboard nav (↑ ↓ ↵ Esc),
// click-outside, and closes on scroll/resize. Options accept: array of strings,
// [value, label] pairs, or { value, label } objects.
import { ref, computed, nextTick, watch, onBeforeUnmount } from 'vue'

const props = defineProps({
  modelValue: { type: [String, Number, Boolean, Object, null], default: '' },
  options: { type: Array, default: () => [] },
  placeholder: { type: String, default: 'Select…' },
  disabled: { type: Boolean, default: false },
  block: { type: Boolean, default: false }, // full-width (else shrinks to content)
  align: { type: String, default: 'left' }, // left | right (menu edge under the button)
})
const emit = defineEmits(['update:modelValue'])

const norm = computed(() =>
  props.options.map((o) => {
    if (Array.isArray(o)) return { value: o[0], label: String(o[1]) }
    if (o && typeof o === 'object') return { value: o.value, label: String(o.label ?? o.value) }
    return { value: o, label: String(o) }
  }),
)
const current = computed(() => norm.value.find((o) => o.value === props.modelValue))
const label = computed(() => current.value?.label ?? props.placeholder)

const open = ref(false)
const active = ref(0)
const btn = ref(null)
const menu = ref(null)
const pos = ref({ left: 0, width: 0, top: null, bottom: null })

const menuStyle = computed(() => ({
  position: 'fixed',
  left: `${pos.value.left}px`,
  minWidth: `${pos.value.width}px`,
  ...(pos.value.top != null ? { top: `${pos.value.top}px` } : { bottom: `${pos.value.bottom}px` }),
}))

function place() {
  const r = btn.value.getBoundingClientRect()
  // Show the whole list when it fits; cap at 80vh so a long list still scrolls.
  const want = Math.min(window.innerHeight * 0.8, norm.value.length * 38 + 12)
  const spaceBelow = window.innerHeight - r.bottom
  const up = spaceBelow < want + 12 && r.top > spaceBelow
  pos.value = {
    left: r.left,
    width: r.width,
    top: up ? null : r.bottom + 6,
    bottom: up ? window.innerHeight - r.top + 6 : null,
  }
}
async function openMenu() {
  if (props.disabled) return
  place()
  open.value = true
  active.value = Math.max(0, norm.value.findIndex((o) => o.value === props.modelValue))
  await nextTick()
  scrollActive()
}
function close() {
  open.value = false
}
function pick(o) {
  emit('update:modelValue', o.value)
  close()
  btn.value?.focus()
}
function scrollActive() {
  menu.value?.children[active.value]?.scrollIntoView({ block: 'nearest' })
}
function onKey(e) {
  if (!open.value) {
    if (['Enter', ' ', 'ArrowDown'].includes(e.key)) {
      e.preventDefault()
      openMenu()
    }
    return
  }
  if (e.key === 'ArrowDown') {
    e.preventDefault()
    active.value = Math.min(norm.value.length - 1, active.value + 1)
    scrollActive()
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    active.value = Math.max(0, active.value - 1)
    scrollActive()
  } else if (e.key === 'Enter') {
    e.preventDefault()
    const o = norm.value[active.value]
    if (o) pick(o)
  } else if (e.key === 'Escape') {
    e.preventDefault()
    close()
  }
}

function onDocClick(e) {
  if (open.value && !btn.value?.contains(e.target) && !menu.value?.contains(e.target)) close()
}
function onScroll() {
  if (open.value) close()
}
watch(open, (v) => {
  if (v) {
    document.addEventListener('click', onDocClick, true)
    window.addEventListener('scroll', onScroll, true)
    window.addEventListener('resize', onScroll)
  } else {
    document.removeEventListener('click', onDocClick, true)
    window.removeEventListener('scroll', onScroll, true)
    window.removeEventListener('resize', onScroll)
  }
})
onBeforeUnmount(() => {
  document.removeEventListener('click', onDocClick, true)
  window.removeEventListener('scroll', onScroll, true)
  window.removeEventListener('resize', onScroll)
})
</script>

<template>
  <div class="relative" :class="block ? 'w-full' : 'inline-block'">
    <button
      ref="btn" type="button" :disabled="disabled" @click="open ? close() : openMenu()" @keydown="onKey"
      class="flex w-full items-center gap-2 rounded-lg border bg-surface2 px-3 py-2 text-sm transition-colors focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
      :class="open ? 'border-accent/70 ring-2 ring-accent/15' : 'border-line hover:border-accent/40'"
    >
      <span class="min-w-0 flex-1 truncate text-left" :class="current ? 'text-fg' : 'text-faint'">{{ label }}</span>
      <svg class="h-4 w-4 shrink-0 text-faint transition-transform" :class="open && 'rotate-180'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m6 9 6 6 6-6"/></svg>
    </button>
    <Teleport to="body">
      <div
        v-if="open" ref="menu" :style="menuStyle"
        class="z-[80] max-h-[80vh] overflow-auto rounded-xl border border-line bg-surface p-1.5 shadow-[0_16px_40px_-12px_rgba(0,0,0,0.7)]"
        :class="align === 'right' && 'origin-top-right'"
      >
        <button
          v-for="(o, i) in norm" :key="i" type="button" @click="pick(o)" @mouseenter="active = i"
          class="flex w-full items-center gap-2 whitespace-nowrap rounded-lg px-2.5 py-2 text-left text-sm"
          :class="[i === active ? 'bg-surface2' : '', o.value === modelValue ? 'text-accent' : 'text-fg']"
        >
          <svg class="h-[15px] w-[15px] shrink-0 text-accent" :class="o.value === modelValue ? 'opacity-100' : 'opacity-0'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6"><path d="M20 6 9 17l-5-5"/></svg>
          <span class="truncate">{{ o.label }}</span>
        </button>
      </div>
    </Teleport>
  </div>
</template>
