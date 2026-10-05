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

<<<<<<< HEAD
const props = defineProps<{
  message: string
  /** Search terms to highlight (plain words only) */
  terms: string[]
=======
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
>>>>>>> feat/books-watcher
}>()

interface HighlightPart {
  text: string
  matched: boolean
}

<<<<<<< HEAD
const parts = computed((): HighlightPart[] => {
  const words = props.terms
    .filter(t => t.length > 0)
    .map(t => t.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'))
  if (words.length === 0) {
    return [{ text: props.message, matched: false }]
  }

  const regex = new RegExp(`(${words.join('|')})`, 'gi')
=======
/** FTS5 token characters under the default unicode61 tokenizer: letter and
 *  number runs. `_` is a SEPARATOR in unicode61 (not part of tokens), so
 *  FTS matches `foo` inside `foo_bar` — the boundary class must match. */
const TOKEN_CHAR = '[\\p{L}\\p{N}]'

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
>>>>>>> feat/books-watcher
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
