<script setup>
import { dismiss, toasts } from '../toast'
</script>

<template>
  <div class="toast-stack" aria-live="polite" aria-atomic="false">
    <div
      v-for="toast in toasts.items"
      :key="toast.id"
      class="toast"
      :class="`toast-${toast.type}`"
      role="status"
    >
      <span>{{ toast.message }}</span>
      <button type="button" class="toast-close" aria-label="Dismiss" @click="dismiss(toast.id)">
        ×
      </button>
    </div>
  </div>
</template>
