<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import Icon from "./Icon.vue";
import type { ActionDialogConfig, DialogField, DialogValues } from "../types";

const props = defineProps<{
  config: ActionDialogConfig;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "submit", values: DialogValues): void;
}>();

const dialogRef = ref<HTMLElement | null>(null);
const validation = ref("");

function initialValue(field: DialogField): string | boolean {
  if (field.kind === "checkbox") return field.value ?? false;
  if (field.kind === "text" || field.kind === "textarea") return field.value ?? "";
  if (field.kind === "select" || field.kind === "radio") return field.value;
  return "";
}

const values = ref<Record<string, string | boolean>>(
  Object.fromEntries(props.config.fields.map((field) => [field.key, initialValue(field)])),
);

const titleId = `dialog-title-${Math.random().toString(36).slice(2, 9)}`;

const requiredFields = computed(() =>
  props.config.fields.filter(
    (field): field is Extract<DialogField, { kind: "text" }> =>
      field.kind === "text" && Boolean(field.required),
  ),
);

function fieldString(field: DialogField): string {
  const value = values.value[field.key];
  return typeof value === "string" ? value : "";
}

function fieldChecked(field: DialogField): boolean {
  return values.value[field.key] === true;
}

function close() {
  emit("close");
}

function submit() {
  for (const field of requiredFields.value) {
    if (!fieldString(field).trim()) {
      validation.value = `请填写${field.label}`;
      return;
    }
  }
  validation.value = "";
  const payload: DialogValues = {};
  for (const field of props.config.fields) {
    if (field.kind === "note") continue;
    const value = values.value[field.key];
    payload[field.key] = typeof value === "boolean" ? value : String(value ?? "").trim();
  }
  emit("submit", payload);
}

function onCheckboxChange(field: DialogField, event: Event) {
  values.value[field.key] = (event.target as HTMLInputElement).checked;
}

function onSelectChange(field: DialogField, event: Event) {
  values.value[field.key] = (event.target as HTMLSelectElement).value;
}

function onTextInput(field: DialogField, event: Event) {
  const target = event.target;
  values.value[field.key] =
    target instanceof HTMLTextAreaElement || target instanceof HTMLInputElement
      ? target.value
      : "";
}

function onRadioChange(field: DialogField, value: string) {
  values.value[field.key] = value;
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    close();
  }
}

onMounted(async () => {
  await nextTick();
  const target =
    dialogRef.value?.querySelector<HTMLElement>("input:not([type='checkbox']), textarea, select") ??
    dialogRef.value?.querySelector<HTMLButtonElement>(".dialog-confirm");
  target?.focus();
  window.addEventListener("keydown", onKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <Teleport to="body">
    <div class="dialog-layer" @pointerdown.self="close">
      <section
        ref="dialogRef"
        class="action-dialog"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="titleId"
        @pointerdown.stop
      >
        <header class="dialog-head">
          <div>
            <h2 :id="titleId">{{ config.title }}</h2>
            <p v-if="config.description">{{ config.description }}</p>
          </div>
          <button type="button" class="icon-button dialog-close" aria-label="关闭" @click="close">
            <Icon name="close" :size="15" />
          </button>
        </header>

        <form @submit.prevent="submit">
          <div class="dialog-body">
            <template v-for="field in config.fields" :key="field.key">
              <label v-if="field.kind === 'text'" class="field dialog-field">
                <span>{{ field.label }}</span>
                <input
                  :value="fieldString(field)"
                  type="text"
                  :placeholder="field.placeholder"
                  :class="{ mono: field.mono }"
                  autocomplete="off"
                  spellcheck="false"
                  @input="onTextInput(field, $event)"
                />
                <small v-if="field.help">{{ field.help }}</small>
              </label>

              <label v-else-if="field.kind === 'textarea'" class="field dialog-field">
                <span>{{ field.label }}</span>
                <textarea
                  :value="fieldString(field)"
                  :placeholder="field.placeholder"
                  rows="3"
                  spellcheck="false"
                  @input="onTextInput(field, $event)"
                ></textarea>
                <small v-if="field.help">{{ field.help }}</small>
              </label>

              <label v-else-if="field.kind === 'select'" class="field dialog-field">
                <span>{{ field.label }}</span>
                <select
                  :value="fieldString(field)"
                  @change="onSelectChange(field, $event)"
                >
                  <option v-for="option in field.options" :key="option.value" :value="option.value">
                    {{ option.label }}
                  </option>
                </select>
                <small v-if="field.help">{{ field.help }}</small>
              </label>

              <label v-else-if="field.kind === 'checkbox'" class="dialog-check">
                <input
                  type="checkbox"
                  :checked="fieldChecked(field)"
                  @change="onCheckboxChange(field, $event)"
                />
                <span class="dialog-check-copy">
                  <strong :class="{ danger: field.danger }">{{ field.label }}</strong>
                  <small v-if="field.help">{{ field.help }}</small>
                </span>
              </label>

              <fieldset v-else-if="field.kind === 'radio'" class="dialog-radio-field">
                <legend v-if="field.label">{{ field.label }}</legend>
                <div class="dialog-radio-list">
                  <label
                    v-for="option in field.options"
                    :key="option.value"
                    class="dialog-radio"
                    :class="{ checked: values[field.key] === option.value }"
                  >
                    <input
                      type="radio"
                      :checked="values[field.key] === option.value"
                      :name="`dialog-radio-${field.key}`"
                      @change="onRadioChange(field, option.value)"
                    />
                    <span>
                      <strong>{{ option.label }}</strong>
                      <small v-if="option.help">{{ option.help }}</small>
                    </span>
                  </label>
                </div>
                <small v-if="field.help">{{ field.help }}</small>
              </fieldset>

              <div
                v-else
                class="dialog-note"
                :class="{ danger: field.danger }"
              >
                <strong v-if="field.label">{{ field.label }}</strong>
                <p>{{ field.text }}</p>
              </div>
            </template>

            <p v-if="validation" class="alert error dialog-validation" role="alert">
              {{ validation }}
            </p>
          </div>

          <footer class="dialog-foot">
            <button type="button" class="button ghost" @click="close">取消</button>
            <button
              type="submit"
              class="button dialog-confirm"
              :class="config.danger ? 'danger' : 'primary'"
            >
              {{ config.confirmLabel ?? "确认" }}
            </button>
          </footer>
        </form>
      </section>
    </div>
  </Teleport>
</template>
