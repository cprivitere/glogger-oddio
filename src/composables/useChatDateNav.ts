import { ref, computed, type Ref, type ComputedRef } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { ChatMessage, ChatDay, ChatFilter } from '../types/database'

/**
 * Shared chat date navigation: play-day list, active day filter, and
 * around-time context loading. Used by every Chat view that renders a
 * ChatMessageList backed by `get_chat_messages`.
 *
 * Day filtering maps the selected day onto `startTime`/`endTime` bounds
 * (`day 00:00:00` … `day 23:59:59`), so the existing offset pagination
 * pipeline works unchanged inside a day.
 */

/** `YYYY-MM-DD` slice of a stored `YYYY-MM-DD HH:MM:SS` timestamp */
export function chatDayOf(timestamp: string): string {
  return timestamp.length >= 10 ? timestamp.slice(0, 10) : timestamp
}

export interface ChatDateNav {
  /** Play days (UTC `YYYY-MM-DD`) with message counts, newest first */
  days: Ref<ChatDay[]>
  /** Currently selected day, or null for live/unfiltered browsing */
  activeDay: Ref<string | null>
  /** True when a day filter is active (viewing the past) */
  isPastView: ComputedRef<boolean>
  /** `startTime`/`endTime` derived from activeDay */
  dayFilter: ComputedRef<{ startTime: string, endTime: string } | null>
  loadDays: () => Promise<void>
  jumpToDay: (day: string | null) => void
  /** `YYYY-MM-DD` of a message timestamp */
  dayOf: (timestamp: string) => string
  /** dir 1 = older day, dir -1 = newer day */
  stepDay: (dir: 1 | -1) => void
  canStepDay: (dir: 1 | -1) => boolean
  /** Returns dayFilter fields to merge into a ChatFilter */
  filterParams: () => { startTime?: string, endTime?: string }
}

export function useChatDateNav(): ChatDateNav {
  const days = ref<ChatDay[]>([])
  const activeDay = ref<string | null>(null)

  async function loadDays() {
    try {
      days.value = await invoke<ChatDay[]>('get_chat_days')
      // Drop the active day if it vanished (e.g. admin purge)
      if (activeDay.value && !days.value.some(d => d.day === activeDay.value)) {
        activeDay.value = null
      }
    } catch (e) {
      console.error('Failed to load chat days:', e)
    }
  }

  const isPastView = computed(() => activeDay.value !== null)

  const dayFilter = computed(() => {
    if (!activeDay.value) return null
    return {
      startTime: `${activeDay.value} 00:00:00`,
      endTime: `${activeDay.value} 23:59:59`,
    }
  })

  function filterParams(): { startTime?: string, endTime?: string } {
    return dayFilter.value ?? {}
  }

  function stepDay(dir: 1 | -1) {
    if (!activeDay.value || days.value.length === 0) return
    const idx = days.value.findIndex(d => d.day === activeDay.value)
    if (idx === -1) return
    const next = days.value[idx + dir] // days are newest-first
    if (next) activeDay.value = next.day
  }

  function canStepDay(dir: 1 | -1): boolean {
    if (!activeDay.value) return false
    const idx = days.value.findIndex(d => d.day === activeDay.value)
    if (idx === -1) return false
    // dir -1 = newer day (earlier in the newest-first list)
    return dir === -1 ? idx > 0 : idx < days.value.length - 1
  }

  function jumpToDay(day: string | null) {
    activeDay.value = day
  }

  return {
    days,
    activeDay,
    isPastView,
    dayFilter,
    loadDays,
    jumpToDay,
    dayOf: chatDayOf,
    stepDay,
    canStepDay,
    filterParams,
  }
}

/** Load context around a time anchor (day jump / prev-day navigation). */
export async function fetchMessagesAroundTime(
  anchorTime: string,
  channel?: string | null,
  contextCount = 60,
): Promise<ChatMessage[]> {
  return invoke<ChatMessage[]>('get_chat_messages_around_time', {
    anchorTime,
    channel: channel ?? undefined,
    contextCount,
  })
}

export type { ChatFilter }
