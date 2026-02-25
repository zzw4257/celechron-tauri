<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

const emit = defineEmits(['login-success']);

const username = ref("***REMOVED***");
const password = ref("***REMOVED***");
const status = ref("");
const isLoading = ref(false);

async function login() {
  if (!username.value || !password.value) return;
  isLoading.value = true;
  status.value = "正在认证...";
  try {
    if (!(window as any).__TAURI_INTERNALS__) {
      throw new Error("请通过 Tauri 桌面环境启动");
    }
    const res = await invoke("login_zju_command", { username: username.value, password: password.value });
    status.value = res as string;
    emit('login-success');
  } catch (err: any) {
    status.value = typeof err === "string" ? err : (err.message || "登录失败");
  } finally {
    isLoading.value = false;
  }
}
</script>

<template>
  <div class="login-scene">
    <div class="login-card">
      <!-- Logo -->
      <div class="logo-section">
        <div class="logo-icon">⏱</div>
        <h1 class="logo-text">Celechron</h1>
        <p class="logo-sub">浙大时间管理助手</p>
      </div>

      <!-- Form -->
      <form class="login-form" @submit.prevent="login" novalidate>
        <div class="field">
          <label class="field-label">ZJU 学号</label>
          <div class="field-input-wrap">
            <input
              id="username-input"
              v-model="username"
              type="text"
              class="field-input"
              placeholder="请输入学号"
              autocomplete="username"
            />
          </div>
        </div>

        <div class="field">
          <label class="field-label">密码</label>
          <div class="field-input-wrap">
            <input
              id="password-input"
              v-model="password"
              type="password"
              class="field-input"
              placeholder="请输入密码"
              autocomplete="current-password"
            />
          </div>
        </div>

        <button type="submit" :disabled="isLoading" class="btn-login">
          <span v-if="!isLoading">登录</span>
          <span v-else class="spinner"></span>
        </button>

        <div class="status-row">
          <p v-if="status" class="status-text" :class="{ error: status.includes('失败') || status.includes('Error') || status.includes('error') }">
            {{ status }}
          </p>
        </div>
      </form>
    </div>
  </div>
</template>

<style scoped>
/* ─── Layout ─── */
.login-scene {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  width: 100%;
  padding: 2rem;
}

/* ─── Card — SOLID so no WKWebView blending artifacts ─── */
.login-card {
  width: 100%;
  max-width: 400px;
  border-radius: 24px;
  padding: 2.8rem 2.4rem 2.4rem;
  display: flex;
  flex-direction: column;
  gap: 2rem;

  /* Light mode: crisp white with depth shadow */
  background: #ffffff;
  border: 1px solid rgba(0, 0, 0, 0.07);
  box-shadow:
    0 2px 4px rgba(0,0,0,0.04),
    0 8px 24px rgba(0,0,0,0.10),
    0 24px 64px rgba(0,0,0,0.08);
}

:global(.dark-theme) .login-card {
  background: #1e293b;   /* slate-800 */
  border: 1px solid rgba(255, 255, 255, 0.08);
  box-shadow:
    0 2px 4px rgba(0,0,0,0.2),
    0 8px 24px rgba(0,0,0,0.3),
    0 24px 64px rgba(0,0,0,0.4);
}

/* ─── Logo ─── */
.logo-section {
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.3rem;
}

.logo-icon {
  font-size: 2.4rem;
  line-height: 1;
  margin-bottom: 0.2rem;
}

.logo-text {
  margin: 0;
  font-size: 2rem;
  font-weight: 800;
  letter-spacing: -1px;
  color: #0284c7;           /* sky-600 — always visible */
  line-height: 1.1;
}

:global(.dark-theme) .logo-text {
  color: #38bdf8;           /* sky-400 */
}

.logo-sub {
  margin: 0;
  font-size: 0.875rem;
  color: #64748b;
  font-weight: 500;
}

:global(.dark-theme) .logo-sub {
  color: #94a3b8;
}

/* ─── Form ─── */
.login-form {
  display: flex;
  flex-direction: column;
  gap: 1.2rem;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.field-label {
  font-size: 0.78rem;
  font-weight: 600;
  letter-spacing: 0.5px;
  text-transform: uppercase;
  color: #64748b;
  padding-left: 2px;
}

:global(.dark-theme) .field-label {
  color: #94a3b8;
}

.field-input-wrap {
  position: relative;
}

.field-input {
  width: 100%;
  padding: 0.75rem 1rem;
  font-size: 0.95rem;
  font-family: inherit;
  border-radius: 10px;
  outline: none;
  transition: border-color 0.15s, box-shadow 0.15s;

  /* Light */
  background: #f8fafc;
  border: 1.5px solid #e2e8f0;
  color: #0f172a;
}

.field-input::placeholder { color: #94a3b8; }

.field-input:focus {
  border-color: #0284c7;
  box-shadow: 0 0 0 3px rgba(2, 132, 199, 0.15);
  background: #ffffff;
}

:global(.dark-theme) .field-input {
  background: #0f172a;
  border: 1.5px solid #334155;
  color: #f1f5f9;
}
:global(.dark-theme) .field-input::placeholder { color: #64748b; }
:global(.dark-theme) .field-input:focus {
  border-color: #38bdf8;
  box-shadow: 0 0 0 3px rgba(56, 189, 248, 0.15);
  background: #0f172a;
}

/* ─── Button ─── */
.btn-login {
  width: 100%;
  padding: 0.85rem;
  border-radius: 12px;
  border: none;
  cursor: pointer;
  font-size: 0.95rem;
  font-weight: 700;
  font-family: inherit;
  letter-spacing: 0.2px;
  color: #ffffff;
  background: linear-gradient(135deg, #0ea5e9 0%, #0284c7 100%);
  box-shadow: 0 4px 12px rgba(2, 132, 199, 0.3);
  transition: opacity 0.15s, transform 0.15s, box-shadow 0.15s;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 46px;
  margin-top: 0.4rem;
}

.btn-login:hover:not(:disabled) {
  opacity: 0.92;
  transform: translateY(-1px);
  box-shadow: 0 6px 18px rgba(2, 132, 199, 0.4);
}

.btn-login:active:not(:disabled) {
  transform: translateY(0);
  box-shadow: 0 2px 8px rgba(2, 132, 199, 0.3);
}

.btn-login:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

/* ─── Spinner ─── */
.spinner {
  width: 18px;
  height: 18px;
  border: 2.5px solid rgba(255,255,255,0.4);
  border-top-color: #fff;
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
  display: inline-block;
}

@keyframes spin { to { transform: rotate(360deg); } }

/* ─── Status ─── */
.status-row {
  min-height: 1.2rem;
  text-align: center;
  margin-top: -0.4rem;
}

.status-text {
  margin: 0;
  font-size: 0.83rem;
  font-weight: 500;
  color: #0284c7;
}

.status-text.error {
  color: #dc2626;
}

:global(.dark-theme) .status-text { color: #38bdf8; }
:global(.dark-theme) .status-text.error { color: #f87171; }
</style>
