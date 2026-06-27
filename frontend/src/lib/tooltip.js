// A lightweight tooltip directive: `v-tip="'text'"` replaces the native `title=`
// attribute (which is slow to appear and renders an unstyled browser box).
// One shared element is teleported to <body> so it never gets clipped by a card's
// overflow. Shows on hover (after a short delay) AND on keyboard focus, themed to
// match the app, with an arrow that flips when there isn't room above.
const DELAY = 110

let tipEl = null
let showT = null
let active = null

function ensure() {
  if (!tipEl) {
    tipEl = document.createElement('div')
    tipEl.className = 'vantage-tip'
    document.body.appendChild(tipEl)
  }
  return tipEl
}

function show(el) {
  const text = el.__tip
  if (!text) return
  const tip = ensure()
  tip.textContent = text
  tip.removeAttribute('data-below')
  tip.classList.add('show')
  const r = el.getBoundingClientRect()
  const tr = tip.getBoundingClientRect()
  let top = r.top - tr.height - 9
  if (top < 6) {
    top = r.bottom + 9
    tip.setAttribute('data-below', '')
  }
  let left = r.left + r.width / 2 - tr.width / 2
  left = Math.max(6, Math.min(left, window.innerWidth - tr.width - 6))
  tip.style.top = `${top}px`
  tip.style.left = `${left}px`
  tip.style.setProperty('--ax', `${r.left + r.width / 2 - left}px`)
  active = el
}

function hide() {
  clearTimeout(showT)
  active = null
  if (tipEl) tipEl.classList.remove('show')
}

function onEnter(e) {
  const el = e.currentTarget
  showT = setTimeout(() => show(el), DELAY)
}
function onFocus(e) {
  show(e.currentTarget)
}

let scrollBound = false
function bindScroll() {
  if (!scrollBound) {
    // Capture phase so scrolling any ancestor container dismisses the tip too.
    window.addEventListener('scroll', hide, true)
    scrollBound = true
  }
}

export const tip = {
  mounted(el, binding) {
    el.__tip = binding.value == null ? '' : String(binding.value)
    el.addEventListener('mouseenter', onEnter)
    el.addEventListener('mouseleave', hide)
    el.addEventListener('focus', onFocus)
    el.addEventListener('blur', hide)
    bindScroll()
  },
  updated(el, binding) {
    el.__tip = binding.value == null ? '' : String(binding.value)
    if (active === el) show(el) // live-update a tip that's currently visible
  },
  beforeUnmount(el) {
    if (active === el) hide()
    el.removeEventListener('mouseenter', onEnter)
    el.removeEventListener('mouseleave', hide)
    el.removeEventListener('focus', onFocus)
    el.removeEventListener('blur', hide)
  },
}
