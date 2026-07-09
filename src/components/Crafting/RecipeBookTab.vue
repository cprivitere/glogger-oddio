<template>
  <div class="h-full min-h-0 flex flex-col p-4 gap-3">
    <!-- Controls -->
    <div class="flex items-end gap-4 flex-wrap shrink-0">
      <!-- Crafting skill selector -->
      <div class="flex flex-col gap-1">
        <label class="text-text-dim text-xs">Crafting Skill</label>
        <select v-model="selectedSkill" class="input text-xs w-52" :disabled="skillsLoading">
          <option value="">{{ skillsLoading ? 'Loading skills…' : 'Select a skill…' }}</option>
          <option v-for="skill in craftingSkills" :key="skill" :value="skill">
            {{ skill }}
          </option>
        </select>
      </div>

      <!-- Unlocked / Missing / Combined -->
      <div class="flex flex-col gap-1">
        <label class="text-text-dim text-xs">Show</label>
        <select v-model="filterMode" class="input text-xs w-36">
          <option value="unlocked">Unlocked</option>
          <option value="missing">Missing</option>
          <option value="combined">Combined</option>
        </select>
      </div>

      <!-- Sort -->
      <div class="flex flex-col gap-1">
        <label class="text-text-dim text-xs">Sort</label>
        <select v-model="sortMode" class="input text-xs w-28">
          <option value="level">Level</option>
          <option value="name">Name</option>
          <option value="crafts">Crafts</option>
        </select>
      </div>

      <!-- Search -->
      <div class="flex flex-col gap-1">
        <label class="text-text-dim text-xs">Search</label>
        <div class="relative">
          <input
            v-model="search"
            type="text"
            placeholder="Filter by name…"
            class="input text-xs w-48 pr-6" />
          <button
            v-if="search"
            class="absolute right-1 top-1/2 -translate-y-1/2 text-text-muted hover:text-text-primary bg-transparent border-none cursor-pointer text-xs px-1"
            title="Clear search"
            @click="search = ''">
            &times;
          </button>
        </div>
      </div>

      <!-- Summary -->
      <div v-if="selectedSkill && hasCompletionData" class="ml-auto flex flex-col items-end gap-1">
        <div class="text-xs text-text-muted">
          <span class="text-green-400 font-semibold font-mono">{{ unlockedCount.toLocaleString() }}</span> unlocked
          <span class="text-text-dim mx-1">·</span>
          <span class="text-red-400 font-semibold font-mono">{{ missingCount.toLocaleString() }}</span> missing
          <span class="text-text-dim mx-1">·</span>
          <span class="text-text-primary font-semibold font-mono">{{ totalCount.toLocaleString() }}</span> total
          <span class="text-text-dim ml-1">({{ unlockedPercent }}%)</span>
        </div>
        <div class="w-48 h-1.5 bg-surface-dark rounded-full overflow-hidden">
          <div
            class="h-full bg-green-500 rounded-full transition-all duration-300"
            :style="{ width: unlockedPercent + '%' }" />
        </div>
      </div>
    </div>

    <!-- No character data banner -->
    <div
      v-if="selectedSkill && !hasCompletionData"
      class="shrink-0 text-xs text-amber-300/90 bg-amber-500/10 border border-amber-500/30 rounded px-3 py-2">
      No character data loaded — import a character report (or tail its Player.log) to see which
      recipes you've unlocked. Showing every recipe in this skill for now.
    </div>

    <!-- Body -->
    <div class="flex-1 min-h-0">
      <EmptyState
        v-if="!selectedSkill"
        variant="panel"
        primary="Select a crafting skill"
        secondary="Choose a crafting skill above to see its unlocked and missing recipes." />

      <div v-else class="h-full min-h-0 flex flex-col border border-surface-elevated rounded overflow-hidden">
        <div class="flex-1 min-h-0 overflow-y-auto">
          <div v-if="loadingRecipes" class="p-4">
            <SkeletonLoader variant="text" :lines="10" />
          </div>

          <div v-else-if="displayRecipes.length === 0" class="p-6 text-text-muted text-xs text-center">
            {{ emptyMessage }}
          </div>

          <table v-else class="w-full text-xs">
            <thead class="sticky top-0 bg-surface-dark z-10">
              <tr class="text-text-dim border-b border-border-light">
                <th v-if="filterMode === 'combined'" class="text-left py-1.5 px-2 font-medium w-24">Status</th>
                <th class="text-left py-1.5 px-2 font-medium">Recipe</th>
                <th class="text-right py-1.5 px-2 font-medium w-16">Level</th>
                <th class="text-right py-1.5 px-2 font-medium w-20">Crafts</th>
                <th class="text-right py-1.5 px-2 font-medium w-16">XP</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="row in displayRecipes"
                :key="row.recipe.id"
                class="border-b border-surface-dark hover:bg-surface-elevated/40"
                :class="row.isKnown
                  ? 'border-l-2 border-l-green-500/50'
                  : 'border-l-2 border-l-red-500/40'">
                <td v-if="filterMode === 'combined'" class="py-1 px-2">
                  <span
                    class="inline-block px-1.5 py-0.5 rounded text-[0.6rem] font-semibold uppercase tracking-wide"
                    :class="row.isKnown
                      ? 'bg-green-500/15 text-green-400'
                      : 'bg-red-500/15 text-red-400'">
                    {{ row.isKnown ? 'Unlocked' : 'Missing' }}
                  </span>
                </td>
                <td class="py-1 px-2">
                  <RecipeInline :reference="row.recipe.name" />
                </td>
                <td class="text-right py-1 px-2 text-text-muted font-mono">
                  {{ row.recipe.skill_level_req ?? '—' }}
                </td>
                <td
                  class="text-right py-1 px-2 font-mono"
                  :class="row.completions > 0 ? 'text-text-primary font-semibold' : 'text-text-muted'">
                  {{ row.isKnown ? row.completions.toLocaleString() : '—' }}
                </td>
                <td class="text-right py-1 px-2 text-text-muted font-mono">
                  {{ row.recipe.reward_skill_xp ?? '—' }}
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <!-- Footer count -->
        <div class="shrink-0 border-t border-surface-elevated px-3 py-1.5 text-[0.65rem] text-text-dim bg-surface-dark/50">
          Showing {{ displayRecipes.length.toLocaleString() }} recipe{{ displayRecipes.length === 1 ? '' : 's' }}
          <span v-if="search" class="ml-1">matching "{{ search }}"</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { useGameDataStore } from "../../stores/gameDataStore";
import { useGameStateStore } from "../../stores/gameStateStore";
import type { RecipeInfo } from "../../types/gameData/recipes";
import EmptyState from "../Shared/EmptyState.vue";
import SkeletonLoader from "../Shared/SkeletonLoader.vue";
import RecipeInline from "../Shared/Recipe/RecipeInline.vue";
import { useViewPrefs } from "../../composables/useViewPrefs";

type FilterMode = "unlocked" | "missing" | "combined";
type SortMode = "level" | "name" | "crafts";

const gameData = useGameDataStore();
const gameStateStore = useGameStateStore();

// Persisted UI selections
const { prefs, update } = useViewPrefs("crafting-recipe-book", {
  selectedSkill: "",
  filterMode: "combined" as FilterMode,
  sortMode: "level" as SortMode,
});

const selectedSkill = ref<string>(prefs.value.selectedSkill);
const filterMode = ref<FilterMode>(prefs.value.filterMode);
const sortMode = ref<SortMode>(prefs.value.sortMode);
const search = ref("");

const craftingSkills = ref<string[]>([]);
const skillsLoading = ref(false);
const loadingRecipes = ref(false);

// Raw recipes for the selected skill (status derived live from game state below)
const rawRecipes = ref<RecipeInfo[]>([]);

// ── Derived data ─────────────────────────────────────────────────────────────

/** True once we have any recipe-completion data (a character is loaded/tailed). */
const hasCompletionData = computed(
  () => Object.keys(gameStateStore.recipeCompletions).length > 0,
);

interface RecipeRow {
  recipe: RecipeInfo;
  /** Unlocked = the recipe key is present in the player's completion map (even at 0 crafts). */
  isKnown: boolean;
  completions: number;
}

/** Enrich raw recipes with unlocked/craft status from the live completion map. */
const enriched = computed<RecipeRow[]>(() => {
  const map = gameStateStore.recipeCompletions;
  return rawRecipes.value.map((recipe) => {
    const key = `Recipe_${recipe.id}`;
    return {
      recipe,
      isKnown: key in map,
      completions: map[key] ?? 0,
    };
  });
});

const unlockedCount = computed(() => enriched.value.filter((r) => r.isKnown).length);
const totalCount = computed(() => enriched.value.length);
const missingCount = computed(() => totalCount.value - unlockedCount.value);
const unlockedPercent = computed(() =>
  totalCount.value > 0 ? Math.round((unlockedCount.value / totalCount.value) * 100) : 0,
);

const displayRecipes = computed<RecipeRow[]>(() => {
  let list = enriched.value;

  // Unlocked / Missing filter — only meaningful once we know the player's recipes.
  if (hasCompletionData.value) {
    if (filterMode.value === "unlocked") list = list.filter((r) => r.isKnown);
    else if (filterMode.value === "missing") list = list.filter((r) => !r.isKnown);
  }

  // Name search
  const q = search.value.trim().toLowerCase();
  if (q) list = list.filter((r) => r.recipe.name.toLowerCase().includes(q));

  // Sort
  const sorted = [...list];
  switch (sortMode.value) {
    case "name":
      sorted.sort((a, b) => a.recipe.name.localeCompare(b.recipe.name));
      break;
    case "crafts":
      sorted.sort(
        (a, b) => b.completions - a.completions || a.recipe.name.localeCompare(b.recipe.name),
      );
      break;
    case "level":
    default:
      sorted.sort(
        (a, b) =>
          (a.recipe.skill_level_req ?? 0) - (b.recipe.skill_level_req ?? 0) ||
          a.recipe.name.localeCompare(b.recipe.name),
      );
      break;
  }
  return sorted;
});

const emptyMessage = computed(() => {
  if (totalCount.value === 0) return "No recipes found for this skill.";
  if (search.value.trim()) return "No recipes match your search.";
  if (filterMode.value === "unlocked") return "No unlocked recipes in this skill yet.";
  if (filterMode.value === "missing") return "You've unlocked every recipe in this skill. 🎉";
  return "No recipes to show.";
});

// ── Loading ──────────────────────────────────────────────────────────────────

onMounted(async () => {
  await loadCraftingSkills();
  if (selectedSkill.value && craftingSkills.value.includes(selectedSkill.value)) {
    await loadRecipes();
  } else if (selectedSkill.value) {
    // Persisted skill no longer valid (e.g. CDN changed) — clear it.
    selectedSkill.value = "";
  }
});

async function loadCraftingSkills() {
  skillsLoading.value = true;
  try {
    const allSkills = await gameData.getAllSkills();
    // A crafting skill = has an XP/level table and at least one recipe (mirrors Leveling tab).
    const candidates = allSkills.filter((s) => s.xp_table);
    const results = await Promise.all(
      candidates.map(async (s) => {
        const recipes = await gameData.getRecipesForSkill(s.name);
        return recipes.length > 0 ? s.name : null;
      }),
    );
    craftingSkills.value = results
      .filter((n): n is string => n !== null)
      .sort((a, b) => a.localeCompare(b));
  } catch (e) {
    console.error("[recipe-book] Failed to load crafting skills:", e);
  } finally {
    skillsLoading.value = false;
  }
}

async function loadRecipes() {
  if (!selectedSkill.value) {
    rawRecipes.value = [];
    return;
  }
  loadingRecipes.value = true;
  try {
    rawRecipes.value = await gameData.getRecipesForSkill(selectedSkill.value);
  } catch (e) {
    console.error("[recipe-book] Failed to load recipes:", e);
    rawRecipes.value = [];
  } finally {
    loadingRecipes.value = false;
  }
}

// ── Persist selections + react to changes ────────────────────────────────────

watch(selectedSkill, (v) => {
  update({ selectedSkill: v });
  loadRecipes();
});
watch(filterMode, (v) => update({ filterMode: v }));
watch(sortMode, (v) => update({ sortMode: v }));
</script>
