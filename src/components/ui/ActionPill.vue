<script setup lang="ts">
withDefaults(
  defineProps<{
    active?: boolean;
    disabled?: boolean;
    tone?: 'default' | 'accent' | 'danger' | 'success' | 'warning';
    type?: 'button' | 'submit';
  }>(),
  {
    active: false,
    disabled: false,
    tone: 'default',
    type: 'button',
  },
);

defineEmits<{
  (event: 'click'): void;
}>();
</script>

<template>
  <button
    class="action-pill"
    :class="[{ active, disabled }, tone]"
    :type="type"
    :disabled="disabled"
    @click="$emit('click')"
  >
    <slot />
  </button>
</template>

<style scoped>
.action-pill {
  border: 1px solid var(--border-subtle);
  background: var(--surface-2);
  color: var(--text-primary);
  border-radius: var(--radius-pill);
  min-height: 2.5rem;
  padding: 0.65rem 0.95rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.45rem;
  font: inherit;
  cursor: pointer;
  transition: transform 160ms ease, border-color 160ms ease, background 160ms ease;
}

.action-pill:hover:not(.disabled) {
  transform: translateY(-1px);
  border-color: var(--border-strong);
}

.action-pill.active,
.action-pill.accent {
  background: var(--accent-soft);
  border-color: var(--accent-border);
  color: var(--accent-text);
}

.action-pill.success.active,
.action-pill.success {
  background: var(--success-soft);
  border-color: var(--success-border);
  color: var(--success-text);
}

.action-pill.warning.active,
.action-pill.warning {
  background: var(--warning-soft);
  border-color: var(--warning-border);
  color: var(--warning-text);
}

.action-pill.danger.active,
.action-pill.danger {
  background: var(--danger-soft);
  border-color: var(--danger-border);
  color: var(--danger-text);
}

.action-pill.disabled {
  opacity: 0.5;
  cursor: not-allowed;
  transform: none;
}
</style>
