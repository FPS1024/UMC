<script setup>
import { onMounted, reactive, ref } from 'vue'
import { api } from '../../api'
import { copyText, formatDate } from '../../format'
import { notify } from '../../toast'
import AdminNav from '../../components/AdminNav.vue'

const invites = ref([])
const loading = ref(true)
const busy = ref('')
const error = ref('')
const newCodes = ref([])
const form = reactive({ count: 1, uses: 1, days: 30, note: '' })

const statusClass = {
  active: 'badge-verified',
  revoked: 'badge-status-disabled',
  expired: 'badge-warning',
  exhausted: 'badge-warning',
}

async function load() {
  loading.value = true
  error.value = ''
  try {
    const data = await api.admin.invites(200)
    invites.value = data?.invites || []
  } catch (err) {
    error.value = err.message
  } finally {
    loading.value = false
  }
}

async function create() {
  busy.value = 'create'
  try {
    const data = await api.admin.createInvites({
      count: Number(form.count),
      uses: Number(form.uses),
      days: Number(form.days),
      note: form.note.trim(),
    })
    invites.value = data?.invites || []
    newCodes.value = data?.codes || []
    notify(`${newCodes.value.length} invite code(s) created.`, 'success')
  } catch (err) {
    notify(err.message, 'error')
  } finally {
    busy.value = ''
  }
}

async function revoke(invite) {
  if (!window.confirm(`Revoke invite ${invite.code_prefix}?`)) return
  busy.value = `invite:${invite.id}`
  try {
    await api.admin.revokeInvite(invite.id)
    invites.value = invites.value.map((item) =>
      item.id === invite.id ? { ...item, status: 'revoked' } : item,
    )
    notify('Invite revoked.', 'success')
  } catch (err) {
    notify(err.message, 'error')
  } finally {
    busy.value = ''
  }
}

async function copy(value, label = 'Invite code') {
  try {
    await copyText(value)
    notify(`${label} copied`, 'success', 2200)
  } catch {
    notify('Could not copy to the clipboard', 'error')
  }
}

async function copyAll() {
  if (!newCodes.value.length) return
  await copy(newCodes.value.join('\n'), 'Invite codes')
}

onMounted(load)
</script>

<template>
  <div class="page-stack">
    <AdminNav />
    <section class="page-heading">
      <div>
        <p class="eyebrow">Administration</p>
        <h1>Invite codes</h1>
        <p class="muted">
          Create registration invites for invite-only mode. Full codes are shown once after creation.
        </p>
      </div>
      <div class="heading-actions">
        <button class="btn btn-secondary btn-sm" type="button" :disabled="loading" @click="load">
          Refresh
        </button>
      </div>
    </section>

    <div v-if="newCodes.length" class="alert alert-warning generated-panel">
      <div>
        <strong>New invite codes</strong>
        <span class="muted small">Shown once. Copy them now.</span>
        <div class="codes">
          <code v-for="code in newCodes" :key="code">{{ code }}</code>
        </div>
      </div>
      <div class="generated-actions">
        <button class="btn btn-secondary btn-sm" type="button" @click="copyAll">Copy all</button>
        <button class="btn btn-ghost btn-sm" type="button" @click="newCodes = []">Dismiss</button>
      </div>
    </div>

    <div v-if="error" class="alert alert-error">{{ error }}</div>

    <section class="card">
      <h2>Create invites</h2>
      <form class="form-grid" @submit.prevent="create">
        <label class="field">
          <span>Count</span>
          <input v-model.number="form.count" type="number" min="1" max="50" required />
          <small>1–50 codes per batch.</small>
        </label>
        <label class="field">
          <span>Uses per code</span>
          <input v-model.number="form.uses" type="number" min="1" max="100" required />
        </label>
        <label class="field">
          <span>Valid for days</span>
          <input v-model.number="form.days" type="number" min="1" max="365" required />
        </label>
        <label class="field">
          <span>Note</span>
          <input v-model="form.note" maxlength="200" placeholder="Optional batch note" />
        </label>
        <div class="card-actions" style="grid-column: 1 / -1; margin-top: 0">
          <button class="btn btn-primary btn-sm" type="submit" :disabled="busy === 'create'">
            {{ busy === 'create' ? 'Creating…' : 'Create invites' }}
          </button>
        </div>
      </form>
    </section>

    <section class="card table-card">
      <h2>Recent invites</h2>
      <div v-if="loading" class="loading-row">
        <span class="spinner spinner-sm" aria-hidden="true"></span> Loading invites…
      </div>
      <div v-else-if="invites.length === 0" class="empty-state">No invite codes yet.</div>
      <div v-else class="table-wrap">
        <table class="data-table">
          <thead>
            <tr>
              <th>Code</th>
              <th>Note</th>
              <th>Uses</th>
              <th>Expires</th>
              <th>Status</th>
              <th class="cell-actions">Actions</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="invite in invites" :key="invite.id">
              <td><code class="code-inline">{{ invite.code_prefix }}…</code></td>
              <td>{{ invite.note || '—' }}</td>
              <td>{{ invite.used_count }} / {{ invite.max_uses }}</td>
              <td class="nowrap">{{ formatDate(invite.expires_at) }}</td>
              <td>
                <span class="badge" :class="statusClass[invite.status] || 'badge-warning'">
                  {{ invite.status }}
                </span>
              </td>
              <td class="cell-actions">
                <div v-if="invite.status === 'active'" class="action-row">
                  <button
                    class="btn btn-ghost btn-xs"
                    type="button"
                    :disabled="busy === `invite:${invite.id}`"
                    @click="revoke(invite)"
                  >
                    Revoke
                  </button>
                </div>
                <span v-else class="muted small">—</span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>
  </div>
</template>
