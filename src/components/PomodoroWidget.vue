<script setup lang="ts">
import { ref, computed, onUnmounted } from 'vue';
import { Play, Pause, Square, CheckCircle2 } from 'lucide-vue-next';

const isRunning = ref(false);
const timeLeft = ref(25 * 60); // 25 mins
const totalTime = ref(25 * 60);
let timerInterval: any = null;

const formatTime = computed(() => {
  const m = Math.floor(timeLeft.value / 60).toString().padStart(2, '0');
  const s = (timeLeft.value % 60).toString().padStart(2, '0');
  return `${m}:${s}`;
});

function toggleTimer() {
  if (isRunning.value) {
    clearInterval(timerInterval);
    isRunning.value = false;
  } else {
    isRunning.value = true;
    timerInterval = setInterval(() => {
      if (timeLeft.value > 0) {
        timeLeft.value--;
      } else {
        clearInterval(timerInterval);
        isRunning.value = false;
        // Notify user using Web Notification Api
        if (Notification.permission === 'granted') {
          new Notification('专注时间结束', { body: '休息一下吧！' });
        }
      }
    }, 1000);
  }
}

function resetTimer() {
  clearInterval(timerInterval);
  isRunning.value = false;
  timeLeft.value = totalTime.value;
}

function setFocusTime(mins: number) {
  totalTime.value = mins * 60;
  resetTimer();
}

onUnmounted(() => {
  clearInterval(timerInterval);
});

// Request notifications
if (typeof Notification !== 'undefined' && Notification.permission !== 'granted' && Notification.permission !== 'denied') {
  Notification.requestPermission();
}
</script>

<template>
  <div class="pomodoro-widget glass-panel section-card">
    <div class="pomo-header">
      <h3><CheckCircle2 :size="16" /> 专注番茄钟</h3>
      <div class="presets">
        <button class="preset-btn" @click="setFocusTime(25)">25m</button>
        <button class="preset-btn" @click="setFocusTime(45)">45m</button>
      </div>
    </div>
    <div class="pomo-display" :class="{ running: isRunning }">
      <div class="time">{{ formatTime }}</div>
    </div>
    <div class="pomo-controls">
      <button class="ctrl-btn main-btn" @click="toggleTimer">
        <Pause v-if="isRunning" :size="20"/>
        <Play v-else :size="20"/>
      </button>
      <button class="ctrl-btn stop-btn" @click="resetTimer" :disabled="timeLeft === totalTime">
        <Square :size="16"/>
      </button>
    </div>
  </div>
</template>

<style scoped>
.pomodoro-widget {
  display: flex;
  flex-direction: column;
  gap: 15px;
  align-items: center;
  padding: 1.5rem;
}
.pomo-header {
  display: flex;
  width: 100%;
  justify-content: space-between;
  align-items: center;
}
.pomo-header h3 {
  margin: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 1rem;
  color: #e2e8f0;
}
.presets {
  display: flex;
  gap: 6px;
}
.preset-btn {
  background: rgba(255,255,255,0.05);
  border: 1px solid rgba(255,255,255,0.1);
  color: #94a3b8;
  padding: 4px 10px;
  border-radius: 12px;
  font-size: 0.8rem;
  cursor: pointer;
}
.preset-btn:hover {
  background: rgba(255,255,255,0.1);
  color: #fff;
}
.pomo-display {
  width: 150px;
  height: 150px;
  border-radius: 50%;
  border: 4px solid rgba(255,255,255,0.1);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: border-color 0.3s, box-shadow 0.3s;
}
.pomo-display.running {
  border-color: #38bdf8;
  box-shadow: 0 0 20px rgba(56,189,248,0.2);
}
.time {
  font-size: 3rem;
  font-weight: 800;
  color: #fff;
  font-variant-numeric: tabular-nums;
}
.pomo-controls {
  display: flex;
  gap: 12px;
}
.ctrl-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  cursor: pointer;
  border-radius: 50%;
  transition: transform 0.2s, background 0.2s;
}
.ctrl-btn:active {
  transform: scale(0.95);
}
.main-btn {
  width: 50px;
  height: 50px;
  background: #38bdf8;
  color: #fff;
}
.main-btn:hover { background: #0284c7; }
.stop-btn {
  width: 40px;
  height: 40px;
  background: rgba(239, 68, 68, 0.15);
  color: #ef4444;
  align-self: center;
}
.stop-btn:hover:not(:disabled) {
  background: rgba(239, 68, 68, 0.3);
}
.stop-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
:global(.light-theme) .pomo-header h3 { color: #1e293b; }
:global(.light-theme) .time { color: #1e293b; }
:global(.light-theme) .pomo-display { border-color: rgba(0,0,0,0.1); }
:global(.light-theme) .preset-btn {
  color: #64748b;
  border-color: rgba(0,0,0,0.1);
}
</style>
