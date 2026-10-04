<template>
  <div class="flex flex-col h-full">
    <div class="px-6 py-4 border-b border-border-default flex justify-between items-center bg-surface-base">
      <h2 class="screen-title m-0">All Messages</h2>
      <div class="flex gap-2 items-center">
        <input
          type="text"
          v-model="searchText"
          @input="onSearchInput"
          placeholder="Search messages..."
          class="px-4 py-2 bg-surface-elevated border border-border-light rounded text-text-primary w-75 focus:outline-none focus:border-accent-gold"
        />
        <input
          type="text"
          v-model="itemNameFilter"
          @input="onSearchInput"
          placeholder="Item name..."
          class="px-4 py-2 bg-surface-elevated border border-border-light rounded text-text-primary w-50 focus:outline-none focus:border-accent-gold"
        />
        <label class="flex items-center gap-1.5 text-text-secondary text-sm cursor-pointer">
          <input
            type="checkbox"
            v-model="hasItemLinksFilter"
            @change="onFilterChange"
            class="w-4 h-4 cursor-pointer"
          />
          Item links only
        </label>
        <button
          @click="refresh"
          :disabled="loading"
          class="w-9 h-9 p-0 bg-surface-elevated border border-border-light rounded text-text-primary text-xl cursor-pointer transition-all flex items-center justify-center hover:bg-border-default hover:border-border-hover disabled:opacity-50 disabled:cursor-not-allowed"
          title="Refresh"
        >
          ⟳
        </button>
      </div>
    </div>
    <ChatMessageList
      :messages="messages"
      :loading="loading"
      :has-more="hasMore"
      :show-channel="true"
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
const searchText = ref('')
const itemNameFilter = ref('')
const hasItemLinksFilter = ref(false)
const sortOrder = ref<'asc' | 'desc'>('desc')
const LIMIT = 100

let searchTimeout: number | null = null

async function loadMessages() {
  loading.value = true
  try {
    const filter: ChatFilter = {
      ...dateNav.filterParams(),
      searchText: searchText.value || undefined,
      itemName: itemNameFilter.value || undefined,
      hasItemLinks: hasItemLinksFilter.value || undefined,
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

function onSearchInput() {
  if (searchTimeout) clearTimeout(searchTimeout)
  searchTimeout = window.setTimeout(() => {
    if (dateNav.activeDay.value) {
      loadAroundDay(dateNav.activeDay.value)
    } else {
      offset.value = 0
      hasMore.value = true
      loadMessages()
    }
  }, 300)
}

function onFilterChange() {
  // Item-link filters must respect the day filter like every other trigger.
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
    const result = await fetchMessagesAroundTime(`${day} 12:00:00`, null, 60)
    messages.value = result
    if (result.length === 0) {
      hasMore.value = false
      return
    }
    // Continue in-day paging from the newest loaded row.
    const youngest = result.reduce((a, b) => (a.timestamp > b.timestamp ? a : b))
    const skip = await invoke<number>('count_chat_messages', {
      startTime: `${day} 00:00:00`,
      endTime: youngest.timestamp.slice(0, 19),
    })
    offset.value = skip - 1
    const more = await invoke<ChatMessage[]>('get_chat_messages', {
      startTime: `${day} 00:00:00`,
      endTime: `${day} 23:59:59`,
      limit: 1,
      offset: offset.value + 1,
      sortOrder: 'desc',
    })
    hasMore.value = more.length > 0
    if (more.length === 0) offset.value = 0
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
