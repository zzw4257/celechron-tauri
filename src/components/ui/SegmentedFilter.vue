<script setup lang="ts">
export interface SegmentedFilterOption {
  value: string;
  label: string;
  badge?: string | number;
}

const props = defineProps<{
  modelValue: string;
  options: SegmentedFilterOption[];
}>();

const emit = defineEmits<{
  (event: 'update:modelValue', value: string): void;
}>();

function choose(value: string) {
  if (value !== props.modelValue) {
    emit('update:modelValue', value);
  }
}
</script>

<template>
  <div class="segmented-filter">
    <button
      v-for="option in options"
      :key="option.value"
      type="button"
      class="segmented-filter__item"
      :class="{ active: modelValue === option.value }"
      @click="choose(option.value)"
    >
      <span>{{ option.label }}</span>
      <small v-if="option.badge !== undefined">{{ option.badge }}</small>
    </button>
  </div>
</template>

<style scoped>
.segmented-filter {
  display: inline-flex;
  flex-wrap: wrap;
  gap: 0.45rem;
}

.segmented-filter__item {
  border: 1px solid var(--border-subtle);
  background: var(--surface-2);
  color: var(--text-secondary);
  border-radius: var(--radius-pill);
  min-height: 2.35rem;
  padding: 0.55rem 0.9rem;
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  font: inherit;
  cursor: pointer;
  transition: background 160ms ease, border-color 160ms ease, color 160ms ease;
}

.segmented-filter__item.active {
  background: var(--surface-1);
  border-color: var(--accent-border);
  color: var(--text-primary);
}

.segmented-filter__item small {
  color: inherit;
  opacity: 0.8;
}
</style>
