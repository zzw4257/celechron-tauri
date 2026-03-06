<script setup lang="ts">
defineProps<{
  label: string;
  value: string;
  hint?: string;
  emphasis?: boolean;
}>();
</script>

<template>
  <div class="inline-stat" :class="{ emphasis }">
    <span class="inline-stat__label">{{ label }}</span>
    <strong class="inline-stat__value">{{ value }}</strong>
    <span v-if="hint" class="inline-stat__hint">{{ hint }}</span>
  </div>
</template>

<style scoped>
.inline-stat {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.18rem;
  padding: 0.9rem 0.95rem;
  border-radius: var(--radius-card-sm);
  background: var(--surface-2);
  border: 1px solid var(--border-subtle);
}

.inline-stat.emphasis {
  background: var(--surface-accent);
  border-color: var(--border-strong);
}

.inline-stat__label,
.inline-stat__hint {
  font-size: 0.82rem;
  color: var(--text-secondary);
}

.inline-stat__value {
  font-size: 1.05rem;
  line-height: 1.2;
  color: var(--text-primary);
}
</style>
