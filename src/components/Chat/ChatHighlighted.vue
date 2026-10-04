<template>
  <span>
    <template v-for="(part, index) in parts" :key="index">
      <mark v-if="part.matched" class="bg-accent-gold/30 text-text-primary rounded-sm px-0.5">{{ part.text }}</mark>
      <template v-else>{{ part.text }}</template>
    </template>
  </span>
</template>

<script setup lang="ts">
import { computed } from 'vue'

/** One highlight term + the backend FTS match kind it carries.
 *  - `exact`: matches whole tokens only (word boundary)
 *  - `phrase`: contiguous substring (FTS5 phrases are token sequences;
 *    substring is the closest DOM approximation)
 *  - `prefix`: must start a token (FTS5 `term*`)
 *  - `literal`: malformed token — substring, mirrors the LIKE fallback */
export interface HighlightTerm {
  term: string
  kind: 'exact' | 'phrase' | 'prefix' | 'literal'
}

const props = defineProps<{
  message: string
  /** Search terms to highlight with their FTS match kinds */
  terms: HighlightTerm[]
}>()

interface HighlightPart {
  text: string
  matched: boolean
}

/** FTS5 token characters: alphanumeric runs (the default unicode61
 *  tokenizer splits on everything else). Used to build token-boundary
 *  aware patterns. */
const TOKEN_CHAR = '[\\p{L}\\p{N}_]'

const parts = computed((): HighlightPart[] => {
  const patterns: string[] = []
  for (const { term, kind } of props.terms) {
    if (term.length === 0) continue
    const esc = term.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    switch (kind) {
      case 'exact':
        // Whole token(s): no token chars touching the match on either side
        patterns.push(`(?<!${TOKEN_CHAR})${esc}(?!${TOKEN_CHAR})`)
        break
      case 'prefix':
        // Match must START a token; the token may continue after it
        patterns.push(`(?<!${TOKEN_CHAR})${esc}`)
        break
      case 'phrase':
      case 'literal':
        // Contiguous substring (phrases span token boundaries by design;
        // literals mirror the LIKE fallback)
        patterns.push(esc)
        break
    }
  }
  if (patterns.length === 0) {
    return [{ text: props.message, matched: false }]
  }

  const regex = new RegExp(`(${patterns.join('|')})`, 'giu')
  const result: HighlightPart[] = []
  let lastIndex = 0
  for (const m of props.message.matchAll(regex)) {
    const pos = m.index ?? 0
    if (pos > lastIndex) {
      result.push({ text: props.message.slice(lastIndex, pos), matched: false })
    }
    result.push({ text: m[0], matched: true })
    lastIndex = pos + m[0].length
  }
  if (lastIndex < props.message.length) {
    result.push({ text: props.message.slice(lastIndex), matched: false })
  }
  return result.length > 0 ? result : [{ text: props.message, matched: false }]
})
</script>
