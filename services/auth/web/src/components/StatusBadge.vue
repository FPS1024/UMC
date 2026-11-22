<script setup>
defineProps({
  value: { type: String, required: true },
  kind: { type: String, default: 'status' },
})
</script>

<template>
  <span class="badge" :class="[`badge-${kind}`, `badge-${kind}-${value}`]">{{ value }}</span>
</template>
