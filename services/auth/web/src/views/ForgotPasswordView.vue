<script setup>
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { api } from '../api'

const router = useRouter()
const email = ref('')
const loading = ref(false)
const error = ref('')
const accepted = ref(false)

async function submit() {
  error.value = ''
  loading.value = true
  try {
    await api.forgotPassword(email.value)
    accepted.value = true
  } catch (err) {
    if (err.code === 'email_unavailable') {
      error.value = 'Email delivery is not configured. Contact the server operator.'
    } else {
      error.value = err.message
    }
  } finally {
    loading.value = false
  }
}

function continueToReset() {
  router.push({ name: 'reset-password', query: { email: email.value } })
}
</script>

<template>
  <div class="auth-wrap">
    <section class="auth-card">
      <div class="auth-heading">
        <p class="eyebrow">Account recovery</p>
        <h1>Reset your password</h1>
        <p>We will send a six-digit reset code if the address exists.</p>
      </div>

      <div v-if="accepted" class="form-stack">
        <div class="alert alert-success">
          If the address exists, a reset code was sent. It expires in 10 minutes.
        </div>
        <button class="btn btn-primary btn-block" type="button" @click="continueToReset">
          Enter the reset code
        </button>
      </div>

      <form v-else class="form-stack" @submit.prevent="submit">
        <label class="field">
          <span>Email</span>
          <input v-model.trim="email" type="email" autocomplete="email" required />
        </label>
        <div v-if="error" class="alert alert-error">{{ error }}</div>
        <button class="btn btn-primary btn-block" type="submit" :disabled="loading">
          {{ loading ? 'Sending…' : 'Send reset code' }}
        </button>
      </form>

      <div class="auth-links">
        <RouterLink to="/login">Return to sign in</RouterLink>
        <RouterLink to="/register">Create an account</RouterLink>
      </div>
    </section>
  </div>
</template>
