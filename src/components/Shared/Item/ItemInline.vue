<template>
  <EntityTooltipWrapper
    border-class="border-entity-item/50"
    :interactive="true"
    entity-type="item"
    :entity-reference="reference"
    :entity-label="displayName"
    @hover="loadData"
  >
    <span
      class="inline-flex items-center gap-0.5 cursor-pointer hover:underline text-entity-item font-medium"
      :class="bordered ? 'bg-entity-item/5 border border-entity-item/20 rounded px-1 py-0.5' : ''"
      @click="handleClick"
    >
      <GameIcon v-if="showIcon" :icon-id="itemData?.icon_id" :alt="reference" size="inline" />
      <span>{{ displayName }}</span>
    </span>
    <template #tooltip>
      <ItemTooltip v-if="itemData" :item="itemData" :icon-src="iconSrc" />
    </template>
  </EntityTooltipWrapper>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useGameDataStore } from "../../../stores/gameDataStore";
import { useEntityNavigation } from "../../../composables/useEntityNavigation";
import type { ItemInfo } from "../../../types/gameData";
import EntityTooltipWrapper from "../EntityTooltipWrapper.vue";
import GameIcon from "../GameIcon.vue";
import ItemTooltip from "./ItemTooltip.vue";

const props = withDefaults(defineProps<{
  reference: string;
  /** Fallback resolver for when `reference` (a display name) doesn't resolve —
   *  e.g. crafted items whose decorated name isn't in the CDN. The base item
   *  type still yields a usable icon and tooltip. */
  typeId?: number | null;
  showIcon?: boolean;
  bordered?: boolean;
}>(), {
  typeId: null,
  showIcon: true,
  bordered: false,
});

const store = useGameDataStore();
const { navigateToEntity } = useEntityNavigation();

const itemData = ref<ItemInfo | null>(null);
const iconSrc = ref<string | null>(null);
// True when `reference` itself resolved by name; false when we fell back to
// `typeId`. In the fallback case we keep showing the original (decorated)
// reference rather than the base item's canonical name.
const resolvedByName = ref(false);

const displayName = computed(() =>
  resolvedByName.value ? (itemData.value?.name ?? props.reference) : props.reference,
);

async function loadData() {
  try {
    let item = await store.resolveItem(props.reference);
    resolvedByName.value = !!item;
    if (!item && props.typeId != null) {
      item = await store.resolveItem(props.typeId);
    }
    if (!item) return;
    itemData.value = item;
    if (item.icon_id) {
      const path = await store.getIconPath(item.icon_id);
      iconSrc.value = convertFileSrc(path);
    }
  } catch (e) {
    console.warn(`Failed to resolve item: ${props.reference}`, e);
  }
}

onMounted(loadData);

watch(() => [props.reference, props.typeId], () => {
  itemData.value = null;
  iconSrc.value = null;
  resolvedByName.value = false;
  loadData();
});

function handleClick() {
  const id = itemData.value?.name ?? props.reference;
  navigateToEntity({ type: "item", id });
}
</script>
