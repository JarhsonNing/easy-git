<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import Icon from "./Icon.vue";
import type { MenuItem } from "../types";

const props = defineProps<{
  x: number;
  y: number;
  heading?: string;
  items: MenuItem[];
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "select", id: string): void;
}>();

const menuRef = ref<HTMLElement | null>(null);
const position = ref({ left: "0px", top: "0px" });

function focusables(): HTMLButtonElement[] {
  return Array.from(
    menuRef.value?.querySelectorAll<HTMLButtonElement>(".canvas-menu-item:not(:disabled)") ?? [],
  );
}

function focusEdge(edge: "first" | "last") {
  const items = focusables();
  if (!items.length) return;
  (edge === "first" ? items[0] : items[items.length - 1]).focus();
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    emit("close");
    return;
  }
  if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
  event.preventDefault();
  const items = focusables();
  if (!items.length) return;
  const index = items.indexOf(document.activeElement as HTMLButtonElement);
  const next =
    event.key === "ArrowDown"
      ? items[(index + 1 + items.length) % items.length]
      : items[(index - 1 + items.length) % items.length];
  next.focus();
}

onMounted(async () => {
  await nextTick();
  const rect = menuRef.value?.getBoundingClientRect();
  const padding = 10;
  const width = rect?.width ?? 236;
  const height = rect?.height ?? 280;
  const left = Math.min(props.x, window.innerWidth - width - padding);
  const top = Math.min(props.y, window.innerHeight - height - padding);
  position.value = {
    left: `${Math.max(padding, left)}px`,
    top: `${Math.max(padding, top)}px`,
  };
  window.addEventListener("keydown", onKeydown);
  focusEdge("first");
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <Teleport to="body">
    <div class="canvas-menu-layer" @pointerdown.self="$emit('close')">
      <div
        ref="menuRef"
        class="canvas-menu"
        :class="{ 'has-heading': heading }"
        role="menu"
        :style="position"
        @contextmenu.prevent
        @pointerdown.stop
      >
        <div v-if="heading" class="canvas-menu-heading">{{ heading }}</div>
        <div v-if="heading" class="canvas-menu-sep" role="separator"></div>
        <template v-for="(item, index) in items" :key="item.id ?? `sep-${index}`">
          <div v-if="item.separator" class="canvas-menu-sep" role="separator"></div>
          <button
            v-else
            type="button"
            class="menu-item canvas-menu-item"
            :class="{ danger: item.danger, 'has-hint': item.hint }"
            role="menuitem"
            :disabled="item.disabled"
            @click="$emit('select', item.id ?? '')"
          >
            <Icon v-if="item.icon" :name="item.icon" :size="14" />
            <span class="canvas-menu-copy">
              <span class="canvas-menu-label">{{ item.label }}</span>
              <span v-if="item.hint" class="canvas-menu-hint">{{ item.hint }}</span>
            </span>
          </button>
        </template>
      </div>
    </div>
  </Teleport>
</template>
