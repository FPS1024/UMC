<script setup>
import { onUnmounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { api } from '../api'
import { setSession } from '../auth'
import { notify } from '../toast'

const route = useRoute()
const router = useRouter()

const email = ref(typeof route.query.email === 'string' ? route.query.email : '')
const code = ref('')
const loading = ref(false)
const resending = ref(false)
const error = ref('')
const message = ref('')
const cooldown = ref(0)
let cooldownTimer = null

function startCooldown(seconds = 60) {
  cooldown.value = seconds
  if (cooldownTimer) window.clearInterval(cooldownTimer)
  cooldownTimer = window.setInterval(() => {
    cooldown.value -= 1
    if (cooldown.value <= 0) {
      window.clearInterval(cooldownTimer)
      cooldownTimer = null
    }
  }, 1000)
}

async function submit() {
  error.value = ''
  loading.value = true
  try {
    const data = await api.verifyEmail(email.value, code.value.trim())
    setSession(data)
    notify('Email verified. Welcome.', 'success')
    router.push('/')
  } catch (err) {
    if (err.code === 'invalid_code') {
      error.value = 'The code is invalid or expired. Request a new one.'
    } else {
      error.value = err.message
    }
  } finally {
    loading.value = false
  }
}

async function resend() {
  if (cooldown.value > 0) return
  error.value = ''
  message.value = ''
  resending.value = true
  try {
    await api.resendVerification(email.value)
    message.value = 'If this address needs verification, a new code was sent.'
    startCooldown()
  } catch (err) {
    if (err.code === 'resend_too_soon') {
      startCooldown()
      error.value = 'Wait a minute before requesting another code.'
    } else if (err.code === 'email_unavailable') {
      error.value = 'Email delivery is not configured. Contact the server operator.'
    } else {
      error.value = err.message
    }
  } finally {
    resending.value = false
  }
}

onUnmounted(() => {
  if (cooldownTimer) window.clearInterval(cooldownTimer)
})
</script>

<template>
  <div class="auth-wrap">
    <section class="auth-card">
      <div class="auth-heading">
        <p class="eyebrow">Email verification</p>
        <h1>Check your inbox</h1>
        <p>Enter the six-digit code we sent to your email address.</p>
      </div>

      <div v-if="message" class="alert alert-success">{{ message }}</div>

      <form class="form-stack" @submit.prevent="submit">
        <label class="field">
          <span>Email</span>
          <input v-model.trim="email" type="email" autocomplete="email" required />
        </label>
        <label class="field">
          <span>Verification code</span>
          <input
            v-model.trim="code"
            inputmode="numeric"
            autocomplete="one-time-code"
            maxlength="6"
            pattern="[0-9]{6}"
            placeholder="000000"
            required
          />
        </label>

        <div v-if="error" class="alert alert-error">{{ error }}</div>

        <button class="btn btn-primary btn-block" type="submit" :disabled="loading">
          {{ loading ? 'Verifying…' : 'Verify email' }}
        </button>
        <button
          class="btn btn-ghost btn-block"
          type="button"
          :disabled="resending || cooldown > 0"
          @click="resend"
        >
          <template v-if="resending">Sending…</template>
          <template v-else-if="cooldown > 0">Resend in {{ cooldown }}s</template>
          <template v-else>Resend code</template>
        </button>
      </form>

      <div class="auth-links">
        <RouterLink to="/login">Return to sign in</RouterLink>
      </div>
    </section>
  </div>
</template>
