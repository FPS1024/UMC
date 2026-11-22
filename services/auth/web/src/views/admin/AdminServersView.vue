<script setup>
import { onMounted, reactive, ref } from 'vue'
import { api } from '../../api'
import { formatDate } from '../../format'
import { notify } from '../../toast'
import AdminNav from '../../components/AdminNav.vue'
import StatusBadge from '../../components/StatusBadge.vue'

const servers = ref([])
const joins = ref([])
const loading = ref(true)
const joinsLoading = ref(false)
const busy = ref('')
const error = ref('')
const joinError = ref('')
const joinServerId = ref('')
const form = reactive({ name: '', address: '', description: '' })

async function loadServers() {
  loading.value = true
  error.value = ''
  try {
    const data = await api.admin.servers()
    servers.value = data?.servers || []
  } catch (err) {
    error.value = err.message
  } finally {
    loading.value = false
  }
}

async function loadJoins() {
  joinsLoading.value = true
  joinError.value = ''
  try {
    const data = await api.admin.serverJoins(joinServerId.value.trim(), 100)
    joins.value = data?.joins || []
  } catch (err) {
    joinError.value = err.message
  } finally {
    joinsLoading.value = false
  }
}

async function createServer() {
  busy.value = 'create'
  try {
    const data = await api.admin.createServer({
      name: form.name.trim(),
      address: form.address.trim(),
      description: form.description.trim(),
    })
    servers.value = [data.server, ...servers.value]
    form.name = ''
    form.address = ''
    form.description = ''
    notify('Server created.', 'success')
  } catch (err) {
    notify(err.message, 'error')
  } finally {
    busy.value = ''
  }
}

async function toggleStatus(server) {
  const status = server.status === 'active' ? 'disabled' : 'active'
  if (!window.confirm(`${status === 'active' ? 'Enable' : 'Disable'} ${server.name}?`)) return
  busy.value = `server:${server.id}`
  try {
    const data = await api.admin.setServerStatus(server.id, status)
    servers.value = servers.value.map((item) => (item.id === server.id ? data.server : item))
    notify(status === 'active' ? 'Server enabled.' : 'Server disabled.', 'success')
  } catch (err) {
    notify(err.message, 'error')
  } finally {
    busy.value = ''
  }
}

async function removeServer(server) {
  if (!window.confirm(`Delete ${server.name}? Join history is kept.`)) return
  busy.value = `server:${server.id}`
  try {
    await api.admin.deleteServer(server.id)
    servers.value = servers.value.filter((item) => item.id !== server.id)
    notify('Server deleted.', 'success')
  } catch (err) {
    notify(err.message, 'error')
  } finally {
    busy.value = ''
  }
}

onMounted(() => {
  loadServers()
  loadJoins()
})
</script>

<template>
  <div class="page-stack">
    <AdminNav />
    <section class="page-heading">
      <div>
        <p class="eyebrow">Administration</p>
        <h1>Game servers</h1>
        <p class="muted">
          Registered Minecraft servers and recent Yggdrasil join activity. Only superadmins can
          change this list.
        </p>
      </div>
      <div class="heading-actions">
        <button class="btn btn-secondary btn-sm" type="button" :disabled="loading" @click="loadServers">
          Refresh
        </button>
      </div>
    </section>

    <div v-if="error" class="alert alert-error">{{ error }}</div>

    <section class="card">
      <h2>Add a server</h2>
      <form class="form-grid" @submit.prevent="createServer">
        <label class="field">
          <span>Name</span>
          <input v-model="form.name" maxlength="64" required placeholder="Survival" />
        </label>
        <label class="field">
          <span>Address</span>
          <input v-model="form.address" maxlength="255" placeholder="mc.example.com:25565" />
        </label>
        <label class="field" style="grid-column: 1 / -1">
          <span>Description</span>
          <input v-model="form.description" maxlength="500" placeholder="Main survival world" />
        </label>
        <div class="card-actions" style="grid-column: 1 / -1; margin-top: 0">
          <button
            class="btn btn-primary btn-sm"
            type="submit"
            :disabled="busy === 'create' || !form.name.trim()"
          >
            {{ busy === 'create' ? 'Creating…' : 'Create server' }}
          </button>
        </div>
      </form>
    </section>

    <section class="card table-card">
      <h2>Registered servers</h2>
      <div v-if="loading" class="loading-row">
        <span class="spinner spinner-sm" aria-hidden="true"></span> Loading servers…
      </div>
      <div v-else-if="servers.length === 0" class="empty-state">No servers registered yet.</div>
      <div v-else class="table-wrap">
        <table class="data-table">
          <thead>
            <tr>
              <th>Server</th>
              <th>Address</th>
              <th>Status</th>
              <th>Created</th>
              <th class="cell-actions">Actions</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="server in servers" :key="server.id">
              <td>
                <div class="cell-stack">
                  <strong>{{ server.name }}</strong>
                  <small>{{ server.description || 'No description' }}</small>
                </div>
              </td>
              <td><code class="code-inline">{{ server.address || '—' }}</code></td>
              <td><StatusBadge :value="server.status" kind="status" /></td>
              <td>{{ formatDate(server.created_at) }}</td>
              <td class="cell-actions">
                <div class="action-row">
                  <button
                    class="btn btn-ghost btn-xs"
                    type="button"
                    :disabled="busy === `server:${server.id}`"
                    @click="toggleStatus(server)"
                  >
                    {{ server.status === 'active' ? 'Disable' : 'Enable' }}
                  </button>
                  <button
                    class="btn btn-ghost btn-xs"
                    type="button"
                    :disabled="busy === `server:${server.id}`"
                    @click="removeServer(server)"
                  >
                    Delete
                  </button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>

    <section class="card table-card">
      <div class="page-heading">
        <div>
          <h2>Join history</h2>
          <p class="muted small">
            Recorded when a launcher authenticates against a game server. Filter by the server id
            sent in the join request.
          </p>
        </div>
        <form class="search-form" @submit.prevent="loadJoins">
          <input
            v-model="joinServerId"
            type="search"
            placeholder="Server id (optional)"
            aria-label="Filter join history by server id"
          />
          <button class="btn btn-primary btn-sm" type="submit" :disabled="joinsLoading">
            Filter
          </button>
        </form>
      </div>
      <div v-if="joinError" class="alert alert-error">{{ joinError }}</div>
      <div v-if="joinsLoading" class="loading-row">
        <span class="spinner spinner-sm" aria-hidden="true"></span> Loading join history…
      </div>
      <div v-else-if="joins.length === 0" class="empty-state">No join records yet.</div>
      <div v-else class="table-wrap">
        <table class="data-table">
          <thead>
            <tr>
              <th>Time</th>
              <th>Player</th>
              <th>Server id</th>
              <th>IP</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="join in joins" :key="join.id">
              <td class="nowrap">{{ formatDate(join.created_at) }}</td>
              <td><strong>{{ join.username }}</strong></td>
              <td><code class="code-inline">{{ join.server_id }}</code></td>
              <td><code class="code-inline">{{ join.ip || '—' }}</code></td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>
  </div>
</template>
