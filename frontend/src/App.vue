<script setup>
import { onMounted, onBeforeUnmount } from 'vue'
import { RouterView } from 'vue-router'

// Animated favicon: a rounded square cycling through the hue spectrum, mirroring
// the sidebar logo and signalling the constant live-data flow.
let timer
onMounted(() => {
  let link = document.querySelector("link[rel~='icon']")
  if (!link) { link = document.createElement('link'); link.rel = 'icon'; document.head.appendChild(link) }
  const c = document.createElement('canvas')
  c.width = c.height = 32
  const ctx = c.getContext('2d')
  let hue = 170 // start near the teal brand color
  const draw = () => {
    hue = (hue + 10) % 360
    ctx.clearRect(0, 0, 32, 32)
    ctx.fillStyle = `hsl(${hue} 70% 55%)`
    if (ctx.roundRect) { ctx.beginPath(); ctx.roundRect(4, 4, 24, 24, 7); ctx.fill() }
    else ctx.fillRect(4, 4, 24, 24)
    link.href = c.toDataURL('image/png')
  }
  draw()
  timer = setInterval(draw, 500)
})
onBeforeUnmount(() => clearInterval(timer))
</script>

<template>
  <RouterView />
</template>
