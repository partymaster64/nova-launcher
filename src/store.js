import { writable } from 'svelte/store'

export const isOnline = writable(true)

export const config = writable(null)
export const accounts = writable([])
export const manifest = writable(null)
export const currentPage = writable('home')
export const launchState = writable({ type: 'idle' })
export const authState = writable({ type: 'idle' })
export const instanceView = writable('grid')
export const modrinthResults = writable(null)

// ID of the instance to show in detail view
export const detailInstanceId = writable(null)

// Which tab to open in detail view ('content' | 'logs' | ...)
export const detailActiveTab = writable('content')

// Per-instance launch states map: { [instanceId]: { type, step, percent, error } }
export const instanceLaunchStates = writable({})

// Per-instance log lines: { [instanceId]: string[] }
export const instanceLogs = writable({})

// Per-instance update info: { [instanceId]: ContentUpdateInfo[] }
// undefined = not yet checked, [] = no updates, [...] = updates available
export const instanceUpdates = writable({})

// Crash event: set when an instance exits abnormally; cleared when the modal is dismissed
// Shape: { instanceId: string, instanceName: string, error: string } | null
export const crashEvent = writable(null)

// Toast notifications: { id, message, type }[]
export const toasts = writable([])
let _toastId = 0
export function addToast(message, type = 'error') {
  const id = ++_toastId
  toasts.update(t => [...t, { id, message, type }])
  setTimeout(() => toasts.update(t => t.filter(x => x.id !== id)), 6000)
}

// Pinned to window so Vite HMR re-executions don't create split instances
if (typeof window !== 'undefined') {
  if (!window.__novaStores) {
    window.__novaStores = { showSetupWizard: writable(false) }
  }
}
export const showSetupWizard =
  typeof window !== 'undefined' ? window.__novaStores.showSetupWizard : writable(false)
