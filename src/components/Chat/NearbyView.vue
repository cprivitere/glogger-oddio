<template>
  <div class="flex flex-col h-full">
    <div class="px-6 py-4 border-b border-border-default flex justify-between items-center bg-surface-base">
      <h2 class="screen-title m-0">Nearby Chat</h2>
      <div class="flex gap-2">
        <button @click="refresh" :disabled="loading" class="w-9 h-9 p-0 bg-surface-elevated border border-border-light rounded text-text-primary text-xl cursor-pointer transition-all flex items-center justify-center hover:bg-border-default hover:border-border-hover disabled:opacity-50 disabled:cursor-not-allowed" title="Refresh">⟳</button>
      </div>
    </div>
    <ChatMessageList
      :messages="messages"
      :loading="loading"
      :has-more="hasMore"
      :sort-order="sortOrder"
      :date-nav="dateNav"
      @load-more="loadMore"
      @toggle-sort="toggleSort"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { ChatMessage, ChatFilter } from '../../types/database'
import ChatMessageList from './ChatMessageList.vue'
import { useChatDateNav, fetchMessagesAroundTime } from '../../composables/useChatDateNav'

const dateNav = useChatDateNav()

const messages = ref<ChatMessage[]>([])
const loading = ref(false)
const hasMore = ref(true)
const offset = ref(0)
const sortOrder = ref<'asc' | 'desc'>('desc')
const LIMIT = 100

async function loadMessages() {
  loading.value = true
  try {
    const filter: ChatFilter = {
      ...dateNav.filterParams(),
      channel: 'Nearby',
      limit: LIMIT,
      offset: offset.value,
      sortOrder: sortOrder.value,
    }

    const newMessages = await invoke<ChatMessage[]>('get_chat_messages', filter)

    if (offset.value === 0) {
      messages.value = newMessages
    } else {
      messages.value = [...messages.value, ...newMessages]
    }

    hasMore.value = newMessages.length === LIMIT
    offset.value += newMessages.length
  } catch (e) {
    console.error('Failed to load messages:', e)
  } finally {
    loading.value = false
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

// Day filter changes reload from the day boundary
watch(() => dateNav.activeDay.value, (day) => {
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
  try {
    const channel = 'Nearby'
    const result = await fetchMessagesAroundTime(`${day} 12:00:00`, channel, 60)
    messages.value = result
    hasMore.value = false
  } catch (e) {
    console.error('Failed to load messages around day:', e)
    offset.value = 0
    hasMore.value = true
    loadMessages()
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  dateNav.loadDays()
  loadMessages()
})
</script>
