<template>
  <div class="flex flex-col h-full">
    <!-- Search header -->
    <div class="px-6 py-4 border-b border-border-default bg-surface-base">
      <div class="flex gap-2 items-center">
        <input
          ref="searchInput"
          type="text"
          v-model="rawQuery"
          @input="onSearchInput"
          placeholder="Search messages... (try from:player or in:channel)"
          class="flex-1 px-4 py-2 bg-surface-elevated border border-border-light rounded text-text-primary focus:outline-none focus:border-accent-gold"
        />
        <button
          @click="refresh"
          :disabled="loading"
          class="w-9 h-9 p-0 bg-surface-elevated border border-border-light rounded text-text-primary text-xl cursor-pointer transition-all flex items-center justify-center hover:bg-border-default hover:border-border-hover disabled:opacity-50 disabled:cursor-not-allowed"
          title="Refresh"
        >
          &#10227;
        </button>
      </div>

      <!-- Active filter chips -->
      <div v-if="parsed.sender || parsed.channel || parsed.textWords.length > 0" class="flex gap-2 mt-2 flex-wrap">
        <span
          v-for="(word, i) in parsed.rawTokens.length > 0 ? parsed.rawTokens : parsed.textWords"
          :key="'text-' + word + '-' + i"
          class="inline-flex items-center gap-1 px-2.5 py-1 bg-text-secondary/15 text-text-primary text-sm rounded-full"
        >
          {{ word }}
          <button
            @click="removeTextWord(word)"
            class="ml-0.5 w-4 h-4 flex items-center justify-center bg-transparent border-none text-text-muted cursor-pointer hover:text-text-primary text-xs leading-none"
          >&times;</button>
        </span>
        <span
          v-if="parsed.sender"
          class="inline-flex items-center gap-1 px-2.5 py-1 bg-accent-blue/15 text-accent-blue text-sm rounded-full"
        >
          from:{{ parsed.sender }}
          <button
            @click="removeOperator('from')"
            class="ml-0.5 w-4 h-4 flex items-center justify-center bg-transparent border-none text-accent-blue/60 cursor-pointer hover:text-accent-blue text-xs leading-none"
          >&times;</button>
        </span>
        <span
          v-if="parsed.channel"
          class="inline-flex items-center gap-1 px-2.5 py-1 bg-accent-gold/15 text-accent-gold text-sm rounded-full"
        >
          in:{{ parsed.channel }}
          <button
            @click="removeOperator('in')"
            class="ml-0.5 w-4 h-4 flex items-center justify-center bg-transparent border-none text-accent-gold/60 cursor-pointer hover:text-accent-gold text-xs leading-none"
          >&times;</button>
        </span>
      </div>

      <!-- Result count -->
      <div v-if="resultCount !== null" class="mt-2 text-xs text-text-muted">
        {{ resultCount.toLocaleString() }} matching message{{ resultCount === 1 ? '' : 's' }} {{ dateNav.activeDay.value ? `on ${dateNav.activeDay.value}` : 'across all history' }}
      </div>
    </div>

    <!-- Context mode header -->
    <div v-if="contextMessageId" class="px-6 py-2 border-b border-border-default bg-surface-elevated flex items-center gap-3">
      <button
        @click="exitContext"
        class="px-3 py-1 bg-surface-base border border-border-light rounded text-text-secondary text-sm cursor-pointer hover:bg-surface-hover hover:text-text-primary transition-all"
      >
        &larr; Back to results
      </button>
      <span class="text-text-muted text-sm">
        Showing context in
        <span v-if="contextChannel" class="font-semibold text-text-secondary">[{{ contextChannel }}]</span>
      </span>
    </div>

    <!-- Results -->
    <ChatMessageList
      :messages="displayMessages"
      :loading="contextMessageId ? contextLoading : loading"
      :has-more="contextMessageId ? false : hasMore"
      :show-channel="!contextMessageId"
      :sort-order="contextMessageId ? undefined : sortOrder"
      :clickable="!contextMessageId"
      :highlight-id="contextMessageId ?? undefined"
      :highlight-terms="contextMessageId ? [] : parsed.highlightTerms"
      :date-nav="dateNav"
      @load-more="loadMore"
      @toggle-sort="toggleSort"
      @message-click="onMessageClick"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, nextTick, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { ChatMessage, ChatFilter } from '../../types/database'
import ChatMessageList from './ChatMessageList.vue'
import { parseSearchQuery } from '../../utils/parseSearchQuery'
import { useChatDateNav, fetchMessagesAroundTime } from '../../composables/useChatDateNav'

const rawQuery = ref('')
const messages = ref<ChatMessage[]>([])
const loading = ref(false)
const hasMore = ref(true)
const offset = ref(0)
const sortOrder = ref<'asc' | 'desc'>('desc')
const searchInput = ref<HTMLInputElement>()
const resultCount = ref<number | null>(null)
const LIMIT = 100

// Date navigation
const dateNav = useChatDateNav()

// Context mode state
const contextMessageId = ref<number | null>(null)
const contextMessages = ref<ChatMessage[]>([])
const contextLoading = ref(false)
const contextChannel = ref<string | null>(null)

let searchTimeout: number | null = null
// Query generation: bumped ONLY when the filter/query changes (page-0
// load). Ordinary pagination (loadMore) must NOT bump it — if it did, a
// page-2 start racing the page-0 count would discard the only count
// response and later pages never request another one, leaving the label
// empty. Query/ page generations are tracked separately.
let queryGeneration = 0
let searchGeneration = 0

const parsed = computed(() => parseSearchQuery(rawQuery.value))

const displayMessages = computed(() =>
  contextMessageId.value ? contextMessages.value : messages.value
)

async function loadMessages() {
  loading.value = true
  // Page-0 loads are new queries: bump the query generation and drop the
  // previous count immediately so the label can't show a stale number.
  // Page-N loads keep the query generation — the in-flight count from the
  // page-0 load must still be able to land.
  const isPage0 = offset.value === 0
  if (isPage0) {
    queryGeneration++
    resultCount.value = null
  }
  const countGeneration = queryGeneration
  const generation = ++searchGeneration
  try {
    const p = parsed.value
    const filter: ChatFilter = {
      searchText: p.text || undefined,
      sender: p.sender || undefined,
      channel: p.channel || undefined,
      ...dateNav.filterParams(),
      limit: LIMIT,
      offset: offset.value,
      sortOrder: sortOrder.value,
    }

    const newMessages = await invoke<ChatMessage[]>('get_chat_messages', filter)

    // A newer search started while this one was in flight: discard.
    if (generation !== searchGeneration) return

    if (offset.value === 0) {
      messages.value = newMessages
    } else {
      messages.value = [...messages.value, ...newMessages]
    }

    hasMore.value = newMessages.length === LIMIT
    offset.value += newMessages.length

    // Count over the full filter (no limit/offset) when viewing page 0
    if (offset.value === newMessages.length) {
      invoke<number>('count_chat_messages', {
        searchText: p.text || undefined,
        sender: p.sender || undefined,
        channel: p.channel || undefined,
        ...dateNav.filterParams(),
      })
        .then(n => {
          if (countGeneration === queryGeneration) resultCount.value = n
        })
        .catch(e => console.error('Failed to count messages:', e))
    }
  } catch (e) {
    console.error('Failed to search messages:', e)
  } finally {
    if (generation === searchGeneration) loading.value = false
  }
}

function loadMore() {
  if (loading.value) return
  loadMessages()
}

function refresh() {
  // With a day filter active, refresh re-centers on that day (the plain
  // day-bounds query can legitimately be empty; around-time keeps context).
  if (dateNav.activeDay.value) {
    loadAroundDay(dateNav.activeDay.value)
  } else {
    offset.value = 0
    hasMore.value = true
    loadMessages()
  }
}

function onSearchInput() {
  if (searchTimeout) clearTimeout(searchTimeout)
  searchTimeout = window.setTimeout(() => {
    exitContext()
    if (dateNav.activeDay.value) {
      loadAroundDay(dateNav.activeDay.value)
    } else {
      offset.value = 0
      hasMore.value = true
      loadMessages()
    }
  }, 300)
}

function toggleSort() {
  sortOrder.value = sortOrder.value === 'desc' ? 'asc' : 'desc'
  if (dateNav.activeDay.value) {
    loadAroundDay(dateNav.activeDay.value)
  } else {
    offset.value = 0
    hasMore.value = true
    loadMessages()
  }
}

function removeTextWord(word: string) {
  // Remove the token exactly as it appears in the query. `\b` boundaries
  // fail on tokens starting/ending with non-word chars (quotes, `*`), so
  // match the escaped token between whitespace boundaries instead.
  const escaped = word.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  rawQuery.value = rawQuery.value
    .replace(new RegExp(`(?:^|(?<=\\s))${escaped}(?:(?=\\s)|$)`, 'i'), '')
    .trim()
    .replace(/\s+/g, ' ')
    .replace(/""/g, '')
  if (dateNav.activeDay.value) {
    loadAroundDay(dateNav.activeDay.value)
  } else {
    offset.value = 0
    hasMore.value = true
    loadMessages()
  }
}

function removeOperator(op: 'from' | 'in') {
  // Remove the operator from the raw query string
  const pattern = op === 'from'
    ? /\bfrom:(?:"[^"]*"|[\S]+)/gi
    : /\bin:(?:"[^"]*"|[\S]+)/gi
  rawQuery.value = rawQuery.value.replace(pattern, '').trim().replace(/\s+/g, ' ')
  if (dateNav.activeDay.value) {
    loadAroundDay(dateNav.activeDay.value)
  } else {
    offset.value = 0
    hasMore.value = true
    loadMessages()
  }
}

async function onMessageClick(msg: ChatMessage) {
  contextMessageId.value = msg.id
  contextChannel.value = msg.channel ?? null
  contextLoading.value = true
  try {
    const result = await invoke<ChatMessage[]>('get_chat_messages_around', {
      messageId: msg.id,
      contextCount: 25,
    })
    contextMessages.value = result
  } catch (e) {
    console.error('Failed to load message context:', e)
    contextMessageId.value = null
  } finally {
    contextLoading.value = false
  }
}

function exitContext() {
  contextMessageId.value = null
  contextMessages.value = []
  contextChannel.value = null
}

// Day filter changes reload from the day boundary
watch(() => dateNav.activeDay.value, (day, prev) => {
  if (day === prev) return
  exitContext()
  if (day) {
    loadAroundDay(day)
  } else {
    offset.value = 0
    hasMore.value = true
    loadMessages()
  }
})

async function loadAroundDay(day: string) {
  loading.value = true
  // A day change is a new query: bump the query generation so any in-flight
  // count from the previous query can't land, then use this generation for
  // the day's own count request.
  queryGeneration++
  const countGeneration = queryGeneration
  const generation = ++searchGeneration
  try {
    // Day window in the current sort order with the complete parsed search
    // filter (text + sender + channel chips) preserved — same filter
    // semantics as loadMessages(). The window is bounded to the day, so
    // continuation is ordinary offset pagination.
    const p = parsed.value
    const result = await fetchMessagesAroundTime(
      day,
      {
        searchText: p.text || undefined,
        sender: p.sender || undefined,
        channel: p.channel || undefined,
        sortOrder: sortOrder.value,
      },
      LIMIT,
    )
    if (generation !== searchGeneration) return
    messages.value = result
    offset.value = result.length
    resultCount.value = null
    if (result.length === 0) {
      hasMore.value = false
      return
    }
    hasMore.value = result.length === LIMIT
    // Recompute the count over the full filter + day bounds.
    invoke<number>('count_chat_messages', {
      searchText: p.text || undefined,
      sender: p.sender || undefined,
      channel: p.channel || undefined,
      startTime: `${day} 00:00:00`,
      endTime: `${day} 23:59:59`,
    })
      .then(n => {
        if (countGeneration === queryGeneration) resultCount.value = n
      })
      .catch(e => console.error('Failed to count messages:', e))
  } catch (e) {
    console.error('Failed to load messages around day:', e)
    // Fall back to day-filtered paging
    offset.value = 0
    hasMore.value = true
    loadMessages()
  } finally {
    if (generation === searchGeneration) loading.value = false
  }
}

onMounted(() => {
  loadMessages()
  dateNav.loadDays()
  nextTick(() => searchInput.value?.focus())
})
</script>
