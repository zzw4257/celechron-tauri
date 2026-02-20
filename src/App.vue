<script setup lang="ts">
import Login from "./components/Login.vue";
</script>

<template>
  <main class="app-container">
    <div class="blob blob-1"></div>
    <div class="blob blob-2"></div>
    <Login />
  </main>
</template>

<style scoped>
.app-container {
  min-height: 100vh;
  width: 100vw;
  position: relative;
  overflow: hidden;
  background-color: #0f172a;
}

/* Background animated blobs for a premium aesthetic */
.blob {
  position: absolute;
  border-radius: 50%;
  filter: blur(80px);
  opacity: 0.6;
  z-index: 0;
  animation: float 20s infinite ease-in-out alternate;
}

.blob-1 {
  width: 400px;
  height: 400px;
  background: radial-gradient(circle, #3b82f6 0%, transparent 70%);
  top: -100px;
  left: -100px;
  animation-delay: 0s;
}

.blob-2 {
  width: 500px;
  height: 500px;
  background: radial-gradient(circle, #8b5cf6 0%, transparent 70%);
  bottom: -150px;
  right: -100px;
  animation-delay: -5s;
}

@keyframes float {
  0% {
    transform: translate(0, 0) scale(1);
  }
  50% {
    transform: translate(50px, 30px) scale(1.1);
  }
  100% {
    transform: translate(-30px, 80px) scale(0.9);
  }
}
</style>