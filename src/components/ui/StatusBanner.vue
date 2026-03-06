<script setup lang="ts">
withDefaults(
  defineProps<{
    tone?: 'info' | 'warning' | 'danger' | 'success';
    title?: string;
  }>(),
  {
    tone: 'info',
    title: '',
  },
);
</script>

<template>
  <div class="status-banner" :class="tone">
    <strong v-if="title">{{ title }}</strong>
    <span><slot /></span>
  </div>
</template>

<style scoped>
.status-banner {
  display: flex;
  gap: 0.55rem;
  align-items: center;
  flex-wrap: wrap;
  padding: 0.85rem 0.95rem;
  border-radius: var(--radius-card-sm);
  border: 1px solid var(--accent-border);
  background: var(--accent-soft);
  color: var(--accent-text);
}

.status-banner.warning {
  background: var(--warning-soft);
  border-color: var(--warning-border);
  color: var(--warning-text);
}

.status-banner.danger {
  background: var(--danger-soft);
  border-color: var(--danger-border);
  color: var(--danger-text);
}

.status-banner.success {
  background: var(--success-soft);
  border-color: var(--success-border);
  color: var(--success-text);
}

.status-banner strong {
  font-size: 0.92rem;
}

.status-banner span {
  font-size: 0.9rem;
}
</style>
