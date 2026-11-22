<script setup>
import { ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { api } from '../api'
import { notify } from '../toast'

const route = useRoute()
const router = useRouter()

const email = ref(typeof route.query.email === 'string' ? route.query.email : '')
const code = ref('')
const password = ref('')
const confirm = ref('')
const loading = ref(false)
const error = ref('')

async function submit() {
  error.value = ''
  if (password.value.length < 8 || password.value.length > 128) {
    error.value = 'Password must be 8–128 characters.'
    return
  }
  if (password.value !== confirm.value) {
    error.value = 'Passwords do not match.'
    return
  }
  loading.value = true
  try {
    await api.resetPassword(email.value, code.value.trim(), password.value)
    notify('Password updated. Sign in with your new password.', 'success')
    router.push({ name: 'login', query: { reset: '1', email: email.value } })
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
</script>

<template>
  <div class="auth-wrap">
    <section class="auth-card">
      <div class="auth-heading">
        <p class="eyebrow">Account recovery</p>
        <h1>Choose a new password</h1>
        <p>Enter the code from your email and pick a new password.</p>
      </div>

      <form class="form-stack" @submit.prevent="submit">
        <label class="field">
          <span>Email</span>
          <input v-model.trim="email" type="email" autocomplete="email" required />
        </label>
        <label class="field">
          <span>Reset code</span>
          <input
            v-model.trim="code"
            inputmode="numeric"
            maxlength="6"
            pattern="[0-9]{6}"
            placeholder="000000"
            required
          />
        </label>
        <label class="field">
          <span>New password</span>
          <input
            v-model="password"
            type="password"
            autocomplete="new-password"
            minlength="8"
            maxlength="128"
            required
          />
        </label>
        <label class="field">
          <span>Confirm new password</span>
          <input
            v-model="confirm"
            type="password"
            autocomplete="new-password"
            required
          />
        </label>

        <div v-if="error" class="alert alert-error">{{ error }}</div>

        <button class="btn btn-primary btn-block" type="submit" :disabled="loading">
          {{ loading ? 'Updating…' : 'Update password' }}
        </button>
      </form>

      <div class="auth-links">
        <RouterLink to="/forgot-password">Request a new code</RouterLink>
        <RouterLink to="/login">Return to sign in</RouterLink>
      </div>
    </section>
  </div>
</template>
