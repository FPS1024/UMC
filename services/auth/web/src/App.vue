<script setup>
import { computed, onMounted } from 'vue'
import { RouterLink, RouterView, useRouter } from 'vue-router'
import { auth, isAdmin, isStaff, logout } from './auth'
import { config, loadMeta } from './config'
import ToastHost from './components/ToastHost.vue'

const router = useRouter()

const serverName = computed(() => config.meta?.server_name || 'Account service')
const initial = computed(() => (auth.user?.username || '?').slice(0, 1).toUpperCase())

async function signOut() {
  await logout()
  router.push({ name: 'login' })
}

onMounted(() => {
  loadMeta()
})
</script>

<template>
  <a class="skip-link" href="#main">Skip to content</a>

  <header class="site-header">
    <div class="shell header-inner">
      <RouterLink to="/" class="brand" aria-label="UMC Auth home">
        <img src="/appicon.png" alt="" width="34" height="34" />
        <span class="brand-text">
          <strong>UMC Auth</strong>
          <small>{{ serverName }}</small>
        </span>
      </RouterLink>

      <nav v-if="auth.user" class="main-nav" aria-label="Main">
        <RouterLink to="/">Dashboard</RouterLink>
        <RouterLink to="/sessions">Sessions</RouterLink>
        <RouterLink to="/profile">Profile</RouterLink>
        <RouterLink v-if="isAdmin" to="/admin">Admin</RouterLink>
        <RouterLink v-else-if="isStaff" to="/admin/users">Staff</RouterLink>
      </nav>

      <div class="header-actions">
        <template v-if="auth.user">
          <span class="user-chip" :title="auth.user.email">
            <span class="avatar">{{ initial }}</span>
            <span class="user-name">{{ auth.user.username }}</span>
          </span>
          <button class="btn btn-ghost btn-sm" type="button" @click="signOut">Sign out</button>
        </template>
        <template v-else>
          <RouterLink class="btn btn-ghost btn-sm" to="/login">Sign in</RouterLink>
          <RouterLink class="btn btn-primary btn-sm" to="/register">Create account</RouterLink>
        </template>
      </div>
    </div>
  </header>

  <main id="main" class="shell main-content">
    <div v-if="!auth.ready" class="loading-panel">
      <span class="spinner" aria-hidden="true"></span>
      Loading your session…
    </div>
    <RouterView v-else />
  </main>

  <footer class="site-footer">
    <div class="shell">
      UMC Auth · community-operated Minecraft account service
    </div>
  </footer>

  <ToastHost />
</template>
