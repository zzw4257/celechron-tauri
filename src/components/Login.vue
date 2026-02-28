<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useAccounts, type SavedAccount } from "../composables/useAccounts";
import { useBiometric } from "../composables/useBiometric";

const emit = defineEmits(['login-success']);

const { accounts, addAccount, getPassword, accountDisplayName } = useAccounts();
const { authenticate } = useBiometric();

const username = ref("");
const password = ref("");
const status = ref("");
const isLoading = ref(false);

// Save Account Modal State
const showSaveModal = ref(false);
const pendingSaveNickname = ref("");
const currentLoginCreds = ref({ username: "", password: "" });

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
    
    // Check if account already exists
    const exists = accounts.value.some(a => a.username === username.value);
    
    if (!exists) {
      currentLoginCreds.value = { username: username.value, password: password.value };
      showSaveModal.value = true;
    } else {
      emit('login-success');
    }
  } catch (err: any) {
    status.value = typeof err === "string" ? err : (err.message || "登录失败");
  } finally {
    isLoading.value = false;
  }
}

async function quickLogin(acc: SavedAccount) {
  if (isLoading.value) return;
  
  // 1. Biometric Auth
  const displayName = accountDisplayName(acc);
  status.value = `等待验证指纹/面容...`;
  
  // Wait a tick so Vue can render the status text before the OS blocks the UI thread
  await new Promise(r => setTimeout(r, 50));
  
  const authOk = await authenticate(displayName);
  
  if (authOk === 'failed') {
    status.value = "系统生物验证已手动取消";
    return;
  }

  const realPwd = await getPassword(acc);
  
  if (authOk === 'fallback') {
    const inputPwd = window.prompt(`请验证身份。\n输入账户 ${displayName} 的密码以继续登录：`);
    if (inputPwd === null) {
      status.value = "已取消身份验证";
      return;
    }
    if (inputPwd !== realPwd) {
      status.value = "密码错误，验证失败";
      return;
    }
  }

  // 2. Decrypt & Login
  isLoading.value = true;
  status.value = `正在登录 ${displayName}...`;
  try {
    const plainPwd = await getPassword(acc);
    const res = await invoke("login_zju_command", { username: acc.username, password: plainPwd });
    status.value = res as string;
    emit('login-success');
  } catch (err: any) {
    status.value = typeof err === "string" ? err : (err.message || "快速登录失败");
  } finally {
    isLoading.value = false;
  }
}

async function confirmSaveAccount() {
  await addAccount(
    currentLoginCreds.value.username, 
    currentLoginCreds.value.password, 
    pendingSaveNickname.value
  );
  showSaveModal.value = false;
  emit('login-success');
}

function skipSaveAccount() {
  showSaveModal.value = false;
  emit('login-success');
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

      <!-- Quick Accounts -->
      <div v-if="accounts.length > 0" class="quick-accounts">
        <p class="section-title">快速登录</p>
        <div class="account-list">
          <button 
            v-for="acc in accounts" 
            :key="acc.id"
            class="account-chip"
            @click="quickLogin(acc)"
            :disabled="isLoading"
          >
            <span class="acc-avatar">{{ (acc.nickname || acc.username).charAt(0).toUpperCase() }}</span>
            <span class="acc-name">{{ accountDisplayName(acc) }}</span>
          </button>
        </div>
        <div class="divider"><span>或使用密码登录</span></div>
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
          <p v-if="status" class="status-text" :class="{ error: status.includes('失败') || status.includes('Error') || status.includes('error') || status.includes('取消') }">
            {{ status }}
          </p>
        </div>
      </form>
    </div>

    <!-- Save Account Modal -->
    <div v-if="showSaveModal" class="modal-overlay">
      <div class="modal-content">
        <h3>保存账户?</h3>
        <p>将账户加密保存到本机，下次可通过指纹或面容快速登录。</p>
        <input 
          v-model="pendingSaveNickname" 
          class="field-input modal-input" 
          placeholder="备注名 (选填，例如：常用号)"
          maxlength="10"
        />
        <div class="modal-actions">
          <button class="btn-cancel" @click="skipSaveAccount">不保存</button>
          <button class="btn-confirm" @click="confirmSaveAccount">保存</button>
        </div>
      </div>
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
/* ─── Quick Accounts ─── */
.quick-accounts {
  display: flex;
  flex-direction: column;
  gap: 0.8rem;
  margin-top: -1rem;
}

.section-title {
  font-size: 0.8rem;
  font-weight: 600;
  color: #64748b;
  margin: 0;
  padding-left: 2px;
}
:global(.dark-theme) .section-title {
  color: #94a3b8;
}

.account-list {
  display: flex;
  gap: 10px;
  overflow-x: auto;
  padding-bottom: 4px;
}
.account-list::-webkit-scrollbar { display: none; }

.account-chip {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 14px 6px 6px;
  background: #f1f5f9;
  border: 1px solid #e2e8f0;
  border-radius: 20px;
  cursor: pointer;
  flex-shrink: 0;
  transition: all 0.2s;
}
:global(.dark-theme) .account-chip {
  background: #334155;
  border: 1px solid #475569;
}
.account-chip:hover:not(:disabled) {
  background: #e2e8f0;
  border-color: #cbd5e1;
}
:global(.dark-theme) .account-chip:hover:not(:disabled) {
  background: #475569;
  border-color: #64748b;
}

.acc-avatar {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background: #0ea5e9;
  color: white;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.9rem;
  font-weight: 700;
}

.acc-name {
  font-size: 0.85rem;
  font-weight: 600;
  color: #334155;
}
:global(.dark-theme) .acc-name {
  color: #e2e8f0;
}

/* ─── Divider ─── */
.divider {
  display: flex;
  align-items: center;
  text-align: center;
  color: #94a3b8;
  font-size: 0.75rem;
  margin: 0.5rem 0;
}
.divider::before, .divider::after {
  content: '';
  flex: 1;
  border-bottom: 1px solid #e2e8f0;
}
.divider span { padding: 0 10px; }
:global(.dark-theme) .divider::before, :global(.dark-theme) .divider::after {
  border-bottom: 1px solid #334155;
}

/* ─── Modal ─── */
.modal-overlay {
  position: absolute;
  top: 0; left: 0; right: 0; bottom: 0;
  background: rgba(0,0,0,0.4);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 24px;
  z-index: 100;
}

.modal-content {
  background: white;
  padding: 1.5rem;
  border-radius: 16px;
  width: 90%;
  max-width: 320px;
  box-shadow: 0 10px 25px rgba(0,0,0,0.1);
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
:global(.dark-theme) .modal-content {
  background: #1e293b;
  border: 1px solid #334155;
}

.modal-content h3 { margin: 0; font-size: 1.2rem; }
.modal-content p { margin: 0; font-size: 0.85rem; color: #64748b; line-height: 1.4; }
:global(.dark-theme) .modal-content h3 { color: white; }
:global(.dark-theme) .modal-content p { color: #94a3b8; }

.modal-input { margin-bottom: 0.5rem; }

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.btn-cancel {
  padding: 8px 16px;
  border-radius: 8px;
  border: none;
  background: transparent;
  color: #64748b;
  font-weight: 600;
  cursor: pointer;
}
.btn-confirm {
  padding: 8px 16px;
  border-radius: 8px;
  border: none;
  background: #0ea5e9;
  color: white;
  font-weight: 600;
  cursor: pointer;
}

@media (max-width: 768px) {
  .login-card { padding: 2rem 1.5rem; max-width: 360px; }
  .logo-icon { font-size: 1.8rem; }
  .logo-text { font-size: 1.5rem; }
  .logo-subtitle { font-size: 0.75rem; }
}
</style>
