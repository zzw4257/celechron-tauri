<script setup lang="ts">
defineProps<{
  title?: string;
  subtitle?: string;
  dense?: boolean;
}>();
</script>

<template>
  <section class="section-card" :class="{ dense }">
    <header v-if="title || subtitle || $slots.header" class="section-card__header">
      <div v-if="title || subtitle" class="section-card__titles">
        <h3 v-if="title">{{ title }}</h3>
        <p v-if="subtitle">{{ subtitle }}</p>
      </div>
      <slot name="header" />
    </header>
    <slot />
  </section>
</template>

<style scoped>
.section-card {
  background: var(--surface-1);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card);
  padding: 1.1rem 1.15rem;
  box-shadow: var(--shadow-soft);
  backdrop-filter: blur(18px);
}

.section-card.dense {
  padding: 0.9rem 1rem;
}

.section-card__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.75rem;
  margin-bottom: 0.9rem;
}

.section-card__titles {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.section-card__titles h3 {
  margin: 0;
  font-size: 1rem;
  color: var(--text-primary);
}

.section-card__titles p {
  margin: 0;
  font-size: 0.9rem;
  color: var(--text-secondary);
}
</style>
