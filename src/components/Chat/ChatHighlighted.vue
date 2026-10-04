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

const props = defineProps<{
  message: string
  /** Search terms to highlight (plain words only) */
  terms: string[]
}>()

interface HighlightPart {
  text: string
  matched: boolean
}

const parts = computed((): HighlightPart[] => {
  const words = props.terms
    .filter(t => t.length > 0)
    .map(t => t.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'))
  if (words.length === 0) {
    return [{ text: props.message, matched: false }]
  }

  const regex = new RegExp(`(${words.join('|')})`, 'gi')
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
