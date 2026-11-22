<script setup>
import { computed, onMounted, ref } from 'vue'
import { api } from '../../api'
import { auth } from '../../auth'
import { formatRelative } from '../../format'
import AdminNav from '../../components/AdminNav.vue'

const stats = ref(null)
const audit = ref([])
const loading = ref(true)
const error = ref('')

const cards = computed(() => {
  if (!stats.value) return []
  return [
    { label: 'Total accounts', value: stats.value.users, tone: 'neutral' },
    { label: 'Active', value: stats.value.active_users, tone: 'success' },
    { label: 'Pending verification', value: stats.value.pending_users, tone: 'warning' },
    { label: 'Disabled', value: stats.value.disabled_users, tone: 'danger' },
    { label: 'Queued mail', value: stats.value.pending_mail, tone: 'accent' },
  ]
})

async function load() {
  loading.value = true
  error.value = ''
  try {
    const [statsData, auditData] = await Promise.all([
      api.admin.stats(),
      api.admin.audit(8),
    ])
    stats.value = statsData
    audit.value = auditData?.entries || []
  } catch (err) {
    error.value = err.message
  } finally {
    loading.value = false
  }
}

onMounted(load)
</script>

<template>
  <div class="page-stack">
    <AdminNav />
    <section class="page-heading">
      <div>
        <p class="eyebrow">Administration</p>
        <h1>Overview</h1>
        <p class="muted">Signed in as {{ auth.user?.username }} · {{ auth.user?.role }}</p>
      </div>
      <div class="heading-actions">
        <button class="btn btn-secondary btn-sm" type="button" @click="load" :disabled="loading">
          Refresh
        </button>
      </div>
    </section>

    <div v-if="error" class="alert alert-error">{{ error }}</div>

    <div v-if="loading" class="loading-row">
      <span class="spinner spinner-sm" aria-hidden="true"></span> Loading statistics…
    </div>

    <template v-else-if="stats">
      <div class="stat-grid">
        <section v-for="card in cards" :key="card.label" class="stat-card" :class="`stat-${card.tone}`">
          <span>{{ card.label }}</span>
          <strong>{{ card.value }}</strong>
        </section>
      </div>

      <div class="card-grid">
        <section class="card">
          <h2>Manage</h2>
          <p class="muted">Account status, roles, verification, password resets, and audit records.</p>
          <div class="card-actions">
            <RouterLink class="btn btn-primary btn-sm" to="/admin/users">Users</RouterLink>
            <RouterLink class="btn btn-secondary btn-sm" to="/admin/audit">Audit log</RouterLink>
            <RouterLink class="btn btn-ghost btn-sm" to="/admin/mail">Mail outbox</RouterLink>
          </div>
        </section>

        <section class="card">
          <h2>Recent activity</h2>
          <div v-if="audit.length === 0" class="empty-state">No audit entries yet.</div>
          <ul v-else class="activity-list">
            <li v-for="entry in audit" :key="entry.id">
              <div>
                <strong>{{ entry.action }}</strong>
                <span class="muted"> · {{ entry.actor }}</span>
                <p v-if="entry.detail" class="muted small">{{ entry.detail }}</p>
              </div>
              <time class="muted small">{{ formatRelative(entry.created_at) }}</time>
            </li>
          </ul>
        </section>
      </div>
    </template>
  </div>
</template>
