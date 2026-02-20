<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

const username = ref("***REMOVED***");
const password = ref("***REMOVED***");
const status = ref("");
const isLoading = ref(false);
const isLoggedIn = ref(false);

async function login() {
  if (!username.value || !password.value) return;
  isLoading.value = true;
  status.value = "Authenticating...";
  try {
    const res = await invoke("login_zju_command", { username: username.value, password: password.value });
    status.value = res as string;
    isLoggedIn.value = true;
  } catch (err: unknown) {
    status.value = typeof err === "string" ? err : "An error occurred during login";
  } finally {
    isLoading.value = false;
  }
}
</script>

<template>
  <div class="login-wrapper">
    <div class="glass-card">
      <div class="logo-container">
        <h1>Celechron</h1>
        <p class="subtitle">Time scheduler for ZJUers</p>
      </div>
      <form class="login-form" @submit.prevent="login" v-if="!isLoggedIn">
        <div class="input-group">
          <label>ZJU ID</label>
          <input id="username-input" v-model="username" type="text" placeholder="Enter ZJU ID..." />
        </div>
        <div class="input-group">
          <label>Password</label>
          <input id="password-input" v-model="password" type="password" placeholder="Enter Password..." />
        </div>
        <button type="submit" :disabled="isLoading" class="btn-primary">
          <span v-if="!isLoading">Login</span>
          <span v-else class="loader"></span>
        </button>
        <p class="status-msg" :class="{ error: status.includes('failed') || status.includes('error') }">{{ status }}</p>
      </form>
      <div v-else class="dashboard-preview">
        <h2>Welcome back!</h2>
        <p>Login was successful. Core features will be displayed here.</p>
        <button @click="isLoggedIn = false; status = ''" class="btn-secondary">Log Out</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.login-wrapper {
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 100vh;
  width: 100%;
}

.glass-card {
  background: rgba(255, 255, 255, 0.05);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
  border: 1px solid rgba(255, 255, 255, 0.1);
  padding: 3rem;
  border-radius: 24px;
  box-shadow: 0 8px 32px 0 rgba(0, 0, 0, 0.3);
  width: 100%;
  max-width: 400px;
  display: flex;
  flex-direction: column;
  gap: 2rem;
  animation: fade-in 0.6s ease-out;
}

.logo-container {
  text-align: center;
}

.logo-container h1 {
  margin: 0;
  font-size: 2.5rem;
  background: linear-gradient(135deg, #00f2fe 0%, #4facfe 100%);
  background-clip: text;
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  font-weight: 800;
  letter-spacing: -1px;
}

.subtitle {
  color: #a0a5b1;
  font-size: 0.95rem;
  margin-top: 0.5rem;
}

.login-form {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.input-group {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  text-align: left;
}

.input-group label {
  font-size: 0.85rem;
  font-weight: 600;
  color: #d1d5db;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

input {
  background: rgba(0, 0, 0, 0.2);
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 12px;
  padding: 1rem;
  color: #fff;
  font-size: 1rem;
  font-family: inherit;
  transition: all 0.3s ease;
  outline: none;
}

input:focus {
  border-color: #4facfe;
  background: rgba(0, 0, 0, 0.4);
  box-shadow: 0 0 0 3px rgba(79, 172, 254, 0.3);
}

.btn-primary {
  background: linear-gradient(135deg, #4facfe 0%, #00f2fe 100%);
  color: #fff;
  border: none;
  border-radius: 12px;
  padding: 1rem;
  font-size: 1rem;
  font-weight: 600;
  cursor: pointer;
  transition: transform 0.2s, box-shadow 0.2s;
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 52px;
}

.btn-primary:hover:not(:disabled) {
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(79, 172, 254, 0.4);
}

.btn-primary:active:not(:disabled) {
  transform: translateY(0);
}

.btn-primary:disabled {
  opacity: 0.7;
  cursor: not-allowed;
}

.btn-secondary {
  background: rgba(255, 255, 255, 0.1);
  color: #fff;
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 12px;
  padding: 0.8rem 1.5rem;
  font-size: 1rem;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-secondary:hover {
  background: rgba(255, 255, 255, 0.15);
}

.loader {
  width: 20px;
  height: 20px;
  border: 3px solid rgba(255, 255, 255, 0.3);
  border-radius: 50%;
  border-top-color: #fff;
  animation: spin 1s ease-in-out infinite;
}

.status-msg {
  font-size: 0.9rem;
  color: #4ade80;
  min-height: 1.2rem;
  text-align: center;
  transition: color 0.3s;
}

.status-msg.error {
  color: #f87171;
}

.dashboard-preview {
  text-align: center;
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
  animation: fade-in 0.4s ease-out;
}

.dashboard-preview h2 {
  color: #4ade80;
  margin: 0;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@keyframes fade-in {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
