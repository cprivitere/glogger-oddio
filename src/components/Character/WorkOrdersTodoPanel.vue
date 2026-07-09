<template>
  <div class="flex flex-col gap-2 flex-1 min-h-0">
    <!-- Toolbar -->
    <div class="flex items-center justify-between gap-2 shrink-0">
      <div class="flex items-center gap-2">
        <input
          v-model="filterText"
          type="text"
          placeholder="Filter by item, name, board..."
          class="px-3 py-1.5 bg-surface-base border border-border-default rounded text-sm text-text-primary placeholder-text-muted focus:outline-none focus:border-accent-gold/50 w-44" />

        <select
          v-model="selectedBoard"
          class="px-2 py-1.5 bg-surface-base border border-border-default rounded text-xs text-text-primary cursor-pointer">
          <option :value="null">All boards</option>
          <option v-for="b in availableBoards" :key="b.name" :value="b.name">
            {{ b.name }} ({{ b.count }})
          </option>
        </select>
      </div>

      <div class="flex items-center gap-3">
        <span class="text-text-dim text-xs">
          {{ selectedCount }} selected · {{ filteredOrders.length }} shown
        </span>
        <button
          class="text-accent-gold text-xs cursor-pointer bg-transparent border border-accent-gold/30 rounded px-2.5 py-1 hover:bg-accent-gold/10 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="creating || selectedCount === 0"
          :title="createTitle"
          @click="createProject">
          {{ creating ? 'Creating…' : `Create Project (${selectedCount})` }}
        </button>
      </div>
    </div>

    <!-- Skill filter pills -->
    <div v-if="availableSkills.length > 0" class="flex items-center gap-2 flex-wrap shrink-0">
      <button
        :class="pillClass(selectedSkill === null)"
        @click="selectedSkill = null">
        All ({{ props.orders.length }})
      </button>
      <button
        v-for="s in availableSkills"
        :key="s.name"
        :class="pillClass(selectedSkill === s.name)"
        @click="selectedSkill = s.name">
        {{ s.name }} ({{ s.count }})
      </button>
    </div>

    <!-- Selection controls -->
    <div
      v-if="filteredOrders.length > 0"
      class="flex items-center justify-between px-1 shrink-0">
      <label class="flex items-center gap-2 text-text-dim text-xs cursor-pointer">
        <input
          type="checkbox"
          class="accent-accent-gold"
          :checked="allFilteredSelected"
          :indeterminate="someFilteredSelected && !allFilteredSelected"
          @change="toggleSelectAll" />
        Select all shown ({{ selectedInFilter }}/{{ filteredOrders.length }})
      </label>
      <button
        v-if="selectedCount > 0"
        class="text-text-muted hover:text-text-primary text-xs cursor-pointer bg-transparent border-0"
        @click="clearSelection">
        Clear selection
      </button>
    </div>

    <!-- Result / error banners -->
    <div
      v-if="resultMsg"
      class="text-[0.7rem] px-2 py-1 rounded border border-green-400/30 bg-green-400/10 text-green-300 shrink-0">
      {{ resultMsg }}
    </div>
    <div v-if="error" class="text-accent-red text-xs shrink-0">{{ error }}</div>

    <!-- List -->
    <div class="flex-1 overflow-y-auto border border-surface-elevated rounded">
      <div v-if="props.orders.length === 0" class="text-text-dim text-xs italic p-4 text-center">
        No uncompleted work orders for your crafting skills.
      </div>
      <div v-else-if="filteredOrders.length === 0" class="text-text-dim text-xs italic p-4 text-center">
        No work orders match your filters.
      </div>

      <label
        v-for="wo in filteredOrders"
        :key="wo.quest_key"
        class="flex items-center gap-3 px-3 py-1.5 border-b border-surface-dark text-xs cursor-pointer hover:bg-surface-row-hover"
        :class="{
          'border-l-2 border-l-green-500/50': wo.is_active,
          'bg-accent-gold/5': selectedSet.has(wo.quest_key),
        }">
        <!-- Checkbox -->
        <input
          v-model="selectedKeys"
          type="checkbox"
          :value="wo.quest_key"
          class="accent-accent-gold shrink-0" />

        <!-- Status -->
        <span v-if="wo.is_active" class="text-green-400 text-[0.6rem] font-semibold shrink-0 w-12">
          ACTIVE
        </span>
        <span v-else class="w-12 shrink-0" />

        <!-- Item & quantity. Rich ItemInline (icon + tooltip, 2 IPC calls each) is
             only used when the list is narrowed enough to afford it; otherwise a
             plain label keeps the full ~1,000-row backlog snappy. -->
        <div class="flex items-center gap-1.5 min-w-0 flex-1">
          <ItemInline
            v-if="richItems && wo.item_internal_name"
            :reference="wo.item_internal_name"
            @click.stop />
          <span v-else class="text-text-secondary truncate">{{ wo.name }}</span>
          <span v-if="wo.quantity" class="text-text-primary font-mono shrink-0">×{{ wo.quantity }}</span>
        </div>

        <!-- Turn-in board -->
        <span
          v-if="wo.board_location"
          class="shrink-0 text-[0.6rem] text-entity-area truncate max-w-32 text-right"
          :title="wo.board_npc ? `Deliver to ${wo.board_npc} — ${wo.board_location}` : wo.board_location">
          {{ wo.board_location }}
        </span>

        <!-- Craft skill -->
        <SkillInline
          v-if="wo.craft_skill"
          :reference="wo.craft_skill"
          :show-icon="true"
          class="shrink-0 text-[0.65rem] w-28"
          @click.stop />

        <!-- Rewards -->
        <div class="flex items-center gap-2 shrink-0 ml-auto w-28 justify-end">
          <span v-if="wo.industry_xp > 0" class="text-accent-gold text-[0.65rem]">
            {{ wo.industry_xp.toLocaleString() }} XP
          </span>
          <span v-if="wo.gold_reward > 0" class="text-yellow-400 text-[0.65rem]">
            {{ wo.gold_reward.toLocaleString() }}g
          </span>
        </div>
      </label>
    </div>

    <!-- Totals -->
    <div
      v-if="filteredOrders.length > 0"
      class="flex items-center gap-4 px-3 py-1.5 bg-surface-elevated rounded text-xs border border-border-light shrink-0">
      <span class="text-text-dim">
        {{ filteredOrders.length }} work order{{ filteredOrders.length !== 1 ? 's' : '' }}
      </span>
      <span v-if="totalXp > 0" class="text-accent-gold">{{ totalXp.toLocaleString() }} Industry XP</span>
      <span v-if="totalGold > 0" class="text-yellow-400">{{ totalGold.toLocaleString() }}g</span>
      <span v-if="activeCount > 0" class="text-green-400 ml-auto">{{ activeCount }} active</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useCraftingStore } from '../../stores/craftingStore'
import type { WorkOrderTodo } from '../../types/crafting'
import ItemInline from '../Shared/Item/ItemInline.vue'
import SkillInline from '../Shared/Skill/SkillInline.vue'

const props = defineProps<{ orders: WorkOrderTodo[] }>()

const craftingStore = useCraftingStore()

const selectedSkill = ref<string | null>(null)
const selectedBoard = ref<string | null>(null)
const filterText = ref('')
const selectedKeys = ref<string[]>([])
const creating = ref(false)
const resultMsg = ref('')
const error = ref('')

function pillClass(active: boolean) {
  return [
    'text-[0.65rem] px-2 py-0.5 rounded border cursor-pointer transition-colors',
    active
      ? 'bg-accent-gold/20 border-accent-gold/40 text-accent-gold'
      : 'bg-transparent border-border-light text-text-muted hover:text-text-primary',
  ]
}

const availableSkills = computed(() => {
  const counts = new Map<string, number>()
  for (const wo of props.orders) {
    const s = wo.craft_skill ?? 'Unknown'
    counts.set(s, (counts.get(s) ?? 0) + 1)
  }
  return [...counts.entries()]
    .map(([name, count]) => ({ name, count }))
    .sort((a, b) => a.name.localeCompare(b.name))
})

const availableBoards = computed(() => {
  const counts = new Map<string, number>()
  for (const wo of props.orders) {
    if (!wo.board_location) continue
    counts.set(wo.board_location, (counts.get(wo.board_location) ?? 0) + 1)
  }
  return [...counts.entries()]
    .map(([name, count]) => ({ name, count }))
    .sort((a, b) => b.count - a.count || a.name.localeCompare(b.name))
})

const filteredOrders = computed(() => {
  let rows = props.orders
  if (selectedSkill.value) rows = rows.filter(wo => (wo.craft_skill ?? 'Unknown') === selectedSkill.value)
  if (selectedBoard.value) rows = rows.filter(wo => wo.board_location === selectedBoard.value)
  const f = filterText.value.trim().toLowerCase()
  if (f) {
    rows = rows.filter(
      wo =>
        wo.name.toLowerCase().includes(f) ||
        (wo.item_internal_name?.toLowerCase().includes(f) ?? false) ||
        (wo.board_location?.toLowerCase().includes(f) ?? false) ||
        (wo.board_npc?.toLowerCase().includes(f) ?? false),
    )
  }
  // Active first, then by name.
  return [...rows].sort((a, b) => {
    if (a.is_active !== b.is_active) return a.is_active ? -1 : 1
    return a.name.localeCompare(b.name)
  })
})

// ── Selection ────────────────────────────────────────────────────────────────
const selectedSet = computed(() => new Set(selectedKeys.value))
const selectedCount = computed(() => selectedKeys.value.length)
const selectedInFilter = computed(
  () => filteredOrders.value.filter(wo => selectedSet.value.has(wo.quest_key)).length,
)
const allFilteredSelected = computed(
  () => filteredOrders.value.length > 0 && selectedInFilter.value === filteredOrders.value.length,
)
const someFilteredSelected = computed(() => selectedInFilter.value > 0)

function toggleSelectAll() {
  const filteredKeys = filteredOrders.value.map(wo => wo.quest_key)
  if (allFilteredSelected.value) {
    // Deselect just the currently-shown rows (keeps any off-filter selections).
    const remove = new Set(filteredKeys)
    selectedKeys.value = selectedKeys.value.filter(k => !remove.has(k))
  } else {
    selectedKeys.value = [...new Set([...selectedKeys.value, ...filteredKeys])]
  }
}

function clearSelection() {
  selectedKeys.value = []
}

// ── Totals ───────────────────────────────────────────────────────────────────
const richItems = computed(() => filteredOrders.value.length <= 250)
const totalXp = computed(() => filteredOrders.value.reduce((s, wo) => s + wo.industry_xp, 0))
const totalGold = computed(() => filteredOrders.value.reduce((s, wo) => s + wo.gold_reward, 0))
const activeCount = computed(() => filteredOrders.value.filter(wo => wo.is_active).length)

const createTitle = computed(() =>
  selectedCount.value === 0
    ? 'Select one or more work orders first'
    : `Create a crafting project from the ${selectedCount.value} selected work order(s). ` +
      'Orders with no craftable recipe are skipped.',
)

async function createProject() {
  const selectedOrders = props.orders.filter(wo => selectedSet.value.has(wo.quest_key))
  if (selectedOrders.length === 0) return
  creating.value = true
  resultMsg.value = ''
  error.value = ''
  try {
    const skillsInSelection = new Set(selectedOrders.map(wo => wo.craft_skill ?? 'Mixed'))
    const skillLabel =
      skillsInSelection.size === 1 ? [...skillsInSelection][0] : (selectedSkill.value ?? 'Selected')
    const name = `Work Orders — ${skillLabel}`
    const notes = `Auto-generated from ${selectedOrders.length} selected work order(s)`
    const { added, skipped } = await craftingStore.createProjectFromWorkOrders(
      selectedOrders,
      name,
      notes,
    )
    if (added === 0) {
      resultMsg.value = `Created "${name}", but none of the ${selectedOrders.length} selected work orders had a craftable recipe.`
    } else {
      resultMsg.value =
        `Created project "${name}" with ${added} recipe${added !== 1 ? 's' : ''}` +
        (skipped > 0 ? ` (${skipped} skipped — no matching recipe).` : '.') +
        ' Open it in Crafting → Projects.'
      clearSelection()
    }
  } catch (e) {
    error.value = String(e)
    console.error('[WorkOrdersTodoPanel] create project failed:', e)
  } finally {
    creating.value = false
  }
}
</script>
