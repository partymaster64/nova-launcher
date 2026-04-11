import { derived } from 'svelte/store'
import { config } from '../store.js'

import de from './locales/de.json'
import en from './locales/en.json'
import fr from './locales/fr.json'

const locales = { de, en, fr }

/** Reactive translation function — use as $t('key') or $t('key', { var: value }) */
export const t = derived(config, ($config) => {
  const lang = $config?.language || 'de'
  const messages = locales[lang] || locales.de

  return (key, vars = {}) => {
    const str = key.split('.').reduce((obj, k) => obj?.[k], messages) ?? key
    return Object.entries(vars).reduce((s, [k, v]) => s.replaceAll(`{${k}}`, v), str)
  }
})
