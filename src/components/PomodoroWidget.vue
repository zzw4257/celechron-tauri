<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue';
import { CheckCircle2, Pause, Play, Square } from 'lucide-vue-next';

const isRunning = ref(false);
const timeLeft = ref(25 * 60);
const totalTime = ref(25 * 60);
let timerInterval: number | null = null;

const progress = computed(() => {
  const total = Math.max(totalTime.value, 1);
  return Math.min(100, Math.max(0, ((total - timeLeft.value) / total) * 100));
});

const formatTime = computed(() => {
  const minutes = Math.floor(timeLeft.value / 60).toString().padStart(2, '0');
  const seconds = (timeLeft.value % 60).toString().padStart(2, '0');
  return `${minutes}:${seconds}`;
});

const dialStyle = computed(() => ({
  background: `conic-gradient(var(--accent-text) ${progress.value}%, color-mix(in srgb, var(--accent-text) 12%, transparent) ${progress.value}% 100%)`,
}));

function stopTimer() {
  if (timerInterval !== null) {
    window.clearInterval(timerInterval);
    timerInterval = null;
  }
}

function toggleTimer() {
  if (isRunning.value) {
    stopTimer();
    isRunning.value = false;
    return;
  }

  isRunning.value = true;
  timerInterval = window.setInterval(() => {
    if (timeLeft.value > 0) {
      timeLeft.value -= 1;
      return;
    }

    stopTimer();
    isRunning.value = false;
    if (typeof Notification !== 'undefined' && Notification.permission === 'granted') {
      new Notification('专注时间结束', { body: '休息一下，再继续。' });
    }
  }, 1000);
}

function resetTimer() {
  stopTimer();
  isRunning.value = false;
  timeLeft.value = totalTime.value;
}

function setFocusTime(minutes: number) {
  totalTime.value = minutes * 60;
  timeLeft.value = totalTime.value;
  stopTimer();
  isRunning.value = false;
}

onUnmounted(stopTimer);

if (typeof Notification !== 'undefined' && Notification.permission === 'default') {
  Notification.requestPermission().catch(() => undefined);
}
</script>

<template>
  <div class="pomodoro-widget">
    <div class="pomo-header">
      <div>
        <h3><CheckCircle2 :size="16" /> 专注番茄钟</h3>
        <p>切成次级组件，但保持可读和可直接开跑。</p>
      </div>
      <div class="presets">
        <button type="button" class="preset-btn" :class="{ active: totalTime === 25 * 60 }" @click="setFocusTime(25)">25m</button>
        <button type="button" class="preset-btn" :class="{ active: totalTime === 45 * 60 }" @click="setFocusTime(45)">45m</button>
      </div>
    </div>

    <div class="pomo-stage">
      <div class="pomo-dial" :style="dialStyle">
        <div class="pomo-dial__inner">
          <span class="pomo-label">{{ isRunning ? '专注中' : '准备开始' }}</span>
          <strong class="time">{{ formatTime }}</strong>
        </div>
      </div>

      <div class="pomo-side">
        <div class="pomo-mini-stat">
          <span>当前时长</span>
          <strong>{{ Math.round(totalTime / 60) }} 分钟</strong>
        </div>
        <div class="pomo-mini-stat">
          <span>进度</span>
          <strong>{{ progress.toFixed(0) }}%</strong>
        </div>
      </div>
    </div>

    <div class="pomo-controls">
      <button type="button" class="ctrl-btn main-btn" @click="toggleTimer">
        <Pause v-if="isRunning" :size="20" />
        <Play v-else :size="20" />
        <span>{{ isRunning ? '暂停' : '开始' }}</span>
      </button>
      <button type="button" class="ctrl-btn stop-btn" :disabled="timeLeft === totalTime" @click="resetTimer">
        <Square :size="16" />
        <span>重置</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.pomodoro-widget {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.pomo-header,
.pomo-stage,
.pomo-controls,
.presets {
  display: flex;
  gap: 0.75rem;
  align-items: center;
}

.pomo-header {
  justify-content: space-between;
}

.pomo-header h3,
.pomo-mini-stat strong,
.time {
  margin: 0;
  color: var(--text-primary);
}

.pomo-header p,
.pomo-mini-stat span,
.pomo-label {
  margin: 0.18rem 0 0;
  color: var(--text-secondary);
}

.preset-btn,
.ctrl-btn {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-pill);
  background: var(--surface-2);
  color: var(--text-primary);
  cursor: pointer;
  transition: transform 160ms ease, border-color 160ms ease, background 160ms ease;
}

.preset-btn {
  min-height: 2rem;
  padding: 0.4rem 0.8rem;
}

.preset-btn.active {
  background: var(--surface-accent);
  border-color: var(--accent-border);
  color: var(--accent-text);
}

.pomo-stage {
  justify-content: space-between;
  align-items: stretch;
}

.pomo-dial {
  width: min(100%, 220px);
  aspect-ratio: 1;
  padding: 10px;
  border-radius: 50%;
  box-shadow: 0 20px 44px color-mix(in srgb, var(--accent-text) 18%, transparent);
}

.pomo-dial__inner {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  background: color-mix(in srgb, var(--surface-1) 94%, transparent);
  border: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.35rem;
}

.time {
  font-size: clamp(2.4rem, 4vw, 3.35rem);
  line-height: 1;
  font-variant-numeric: tabular-nums;
}

.pomo-side {
  flex: 1;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.75rem;
}

.pomo-mini-stat {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: var(--surface-2);
  padding: 0.95rem;
}

.ctrl-btn {
  min-height: 2.75rem;
  padding: 0.65rem 1rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.45rem;
}

.ctrl-btn:hover:not(:disabled),
.preset-btn:hover {
  transform: translateY(-1px);
  border-color: var(--border-strong);
}

.main-btn {
  background: var(--accent-soft);
  border-color: var(--accent-border);
  color: var(--accent-text);
}

.stop-btn {
  background: var(--danger-soft);
  border-color: var(--danger-border);
  color: var(--danger-text);
}

.stop-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
  transform: none;
}

@media (max-width: 720px) {
  .pomo-stage {
    flex-direction: column;
    align-items: center;
  }

  .pomo-side {
    width: 100%;
  }

  .pomo-controls,
  .pomo-header {
    flex-wrap: wrap;
  }
}
</style>
