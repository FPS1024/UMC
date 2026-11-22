<script setup>
import { isAdmin } from '../auth'
</script>

<template>
  <nav class="admin-nav" aria-label="Administration">
    <RouterLink v-if="isAdmin" to="/admin">Overview</RouterLink>
    <RouterLink to="/admin/users">Users</RouterLink>
    <RouterLink to="/admin/audit">Audit</RouterLink>
    <RouterLink v-if="isAdmin" to="/admin/servers">Servers</RouterLink>
    <RouterLink v-if="isAdmin" to="/admin/mail">Mail</RouterLink>
  </nav>
</template>
