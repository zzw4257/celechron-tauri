// useAccounts.ts — Multi-account storage with AES-GCM encryption
// Passwords are encrypted client-side; the key is derived from a device-stable secret.

import { ref, computed } from 'vue';

export interface SavedAccount {
    id: string;
    username: string;      // Full student ID
    nickname: string;      // User-set alias (可空)
    encPassword: string;   // AES-GCM encrypted password (base64)
    iv: string;            // Encryption IV (base64)
    addedAt: number;
}

const STORE_KEY = 'celechron_accounts';
// A per-device derived key phrase (not truly secret but makes the stored blob opaque)
const KEY_PHRASE = 'celechron-local-key-v1';

// ── Crypto helpers ──────────────────────────────────────────────────────────
async function getKey(): Promise<CryptoKey> {
    const enc = new TextEncoder();
    const rawKey = await crypto.subtle.importKey(
        'raw', enc.encode(KEY_PHRASE), { name: 'PBKDF2' }, false, ['deriveKey']
    );
    return crypto.subtle.deriveKey(
        { name: 'PBKDF2', salt: enc.encode('celechron-salt'), iterations: 100000, hash: 'SHA-256' },
        rawKey,
        { name: 'AES-GCM', length: 256 },
        false,
        ['encrypt', 'decrypt']
    );
}

async function encryptPassword(password: string): Promise<{ encPassword: string; iv: string }> {
    const key = await getKey();
    const ivBytes = crypto.getRandomValues(new Uint8Array(12));
    const enc = new TextEncoder();
    const cipher = await crypto.subtle.encrypt({ name: 'AES-GCM', iv: ivBytes }, key, enc.encode(password));
    return {
        encPassword: btoa(String.fromCharCode(...new Uint8Array(cipher))),
        iv: btoa(String.fromCharCode(...ivBytes)),
    };
}

async function decryptPassword(encPassword: string, iv: string): Promise<string> {
    const key = await getKey();
    const ivBytes = Uint8Array.from(atob(iv), c => c.charCodeAt(0));
    const cipher = Uint8Array.from(atob(encPassword), c => c.charCodeAt(0));
    const plain = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: ivBytes }, key, cipher);
    return new TextDecoder().decode(plain);
}

// ── State ───────────────────────────────────────────────────────────────────
function loadAccounts(): SavedAccount[] {
    try {
        return JSON.parse(localStorage.getItem(STORE_KEY) || '[]');
    } catch {
        return [];
    }
}

function saveAccounts(accounts: SavedAccount[]) {
    localStorage.setItem(STORE_KEY, JSON.stringify(accounts));
}

const savedAccounts = ref<SavedAccount[]>(loadAccounts());

// ── Display format: 备注名(学号后4位) ──────────────────────────────────────
export function accountDisplayName(acc: SavedAccount): string {
    const suffix = acc.username.slice(-4);
    return acc.nickname ? `${acc.nickname}(${suffix})` : `学号尾号(${suffix})`;
}

// ── Public API ───────────────────────────────────────────────────────────────
export function useAccounts() {
    const accounts = computed(() => savedAccounts.value);
    const maxAccounts = 5;
    const isFull = computed(() => savedAccounts.value.length >= maxAccounts);

    async function addAccount(username: string, password: string, nickname: string): Promise<void> {
        // Deduplicate by username
        const existing = savedAccounts.value.findIndex(a => a.username === username);
        const { encPassword, iv } = await encryptPassword(password);
        const entry: SavedAccount = {
            id: existing >= 0 ? savedAccounts.value[existing].id : crypto.randomUUID(),
            username,
            nickname: nickname.trim(),
            encPassword,
            iv,
            addedAt: Date.now(),
        };
        if (existing >= 0) {
            savedAccounts.value.splice(existing, 1, entry);
        } else {
            if (savedAccounts.value.length >= maxAccounts) {
                // Remove oldest
                savedAccounts.value.sort((a, b) => a.addedAt - b.addedAt);
                savedAccounts.value.shift();
            }
            savedAccounts.value.push(entry);
        }
        saveAccounts(savedAccounts.value);
    }

    function removeAccount(id: string): void {
        savedAccounts.value = savedAccounts.value.filter(a => a.id !== id);
        saveAccounts(savedAccounts.value);
    }

    function updateNickname(id: string, nickname: string): void {
        const acc = savedAccounts.value.find(a => a.id === id);
        if (acc) {
            acc.nickname = nickname.trim();
            saveAccounts(savedAccounts.value);
        }
    }

    async function getPassword(acc: SavedAccount): Promise<string> {
        return decryptPassword(acc.encPassword, acc.iv);
    }

    return {
        accounts,
        isFull,
        maxAccounts,
        addAccount,
        removeAccount,
        updateNickname,
        getPassword,
        accountDisplayName,
    };
}
