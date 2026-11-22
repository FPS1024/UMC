<script setup>
import { computed, reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { api } from '../api'
import { setSession } from '../auth'

const route = useRoute()
const router = useRouter()

const form = reactive({
  login: typeof route.query.email === 'string' ? route.query.email : '',
  password: '',
})
const loading = ref(false)
const error = ref('')
const errorCode = ref('')
const showPassword = ref(false)

const notice = computed(() => {
  if (route.query.verified === '1') return 'Email verified. You can sign in now.'
  if (route.query.reset === '1') return 'Password updated. Sign in with your new password.'
  return ''
})

async function submit() {
  error.value = ''
  errorCode.value = ''
  loading.value = true
  try {
    const data = await api.login(form.login, form.password)
    setSession(data)
    const redirect = typeof route.query.redirect === 'string' ? route.query.redirect : '/'
    router.push(redirect.startsWith('/') ? redirect : '/')
  } catch (err) {
    error.value = err.message
    errorCode.value = err.code || ''
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="auth-wrap">
    <section class="auth-card">
      <div class="auth-heading">
        <p class="eyebrow">Account</p>
        <h1>Welcome back</h1>
        <p>Sign in with your username or email address.</p>
      </div>

      <div v-if="notice" class="alert alert-success">{{ notice }}</div>

      <form class="form-stack" @submit.prevent="submit">
        <label class="field">
          <span>Username or email</span>
          <input
            v-model.trim="form.login"
            autocomplete="username"
            autofocus
            required
            @input="error = ''"
          />
        </label>

        <label class="field">
          <span>Password</span>
          <span class="input-with-action">
            <input
              v-model="form.password"
              :type="showPassword ? 'text' : 'password'"
              autocomplete="current-password"
              required
              @input="error = ''"
            />
            <button
              type="button"
              class="input-action"
              @click="showPassword = !showPassword"
            >
              {{ showPassword ? 'Hide' : 'Show' }}
            </button>
          </span>
        </label>

        <div v-if="error" class="alert alert-error">
          {{ error }}
          <RouterLink
            v-if="errorCode === 'email_not_verified'"
            :to="{ name: 'verify-email', query: { email: form.login } }"
          >
            Verify email
          </RouterLink>
        </div>

        <button class="btn btn-primary btn-block" type="submit" :disabled="loading">
          {{ loading ? 'Signing in…' : 'Sign in' }}
        </button>
      </form>

      <div class="auth-links">
        <RouterLink to="/forgot-password">Forgot password?</RouterLink>
        <RouterLink to="/register">Create an account</RouterLink>
      </div>
    </section>
  </div>
</template>
