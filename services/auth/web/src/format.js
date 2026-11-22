export function formatDate(value) {
  if (!value) return '—'
  const date = new Date(value * 1000)
  if (Number.isNaN(date.getTime())) return '—'
  return date.toLocaleString(undefined, {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

export function formatDateOnly(value) {
  if (!value) return '—'
  const date = new Date(value * 1000)
  if (Number.isNaN(date.getTime())) return '—'
  return date.toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
  })
}

export function formatRelative(value) {
  if (!value) return '—'
  const seconds = Math.round(Date.now() / 1000 - value)
  if (seconds < 60) return 'just now'
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`
  if (seconds < 86400 * 30) return `${Math.floor(seconds / 86400)}d ago`
  return formatDateOnly(value)
}

export function prettyUuid(uuid) {
  if (!uuid) return ''
  const normalized = uuid.replaceAll('-', '')
  if (normalized.length !== 32) return uuid
  return [
    normalized.slice(0, 8),
    normalized.slice(8, 12),
    normalized.slice(12, 16),
    normalized.slice(16, 20),
    normalized.slice(20),
  ].join('-')
}

export function shortUuid(uuid) {
  if (!uuid) return ''
  return `${uuid.slice(0, 8)}…${uuid.slice(-4)}`
}

export async function copyText(text) {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text)
    return true
  }
  const element = document.createElement('textarea')
  element.value = text
  element.setAttribute('readonly', '')
  element.style.position = 'fixed'
  element.style.opacity = '0'
  document.body.appendChild(element)
  element.select()
  const copied = document.execCommand('copy')
  document.body.removeChild(element)
  return copied
}
