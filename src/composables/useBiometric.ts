import { authenticate as tauriAuthenticate, checkStatus } from '@tauri-apps/plugin-biometric';

export function useBiometric() {
    /**
     * Check if the platform has a native biometric authenticator available.
     */
    async function isBiometricAvailable(): Promise<boolean> {
        try {
            const status = await checkStatus();
            return status.isAvailable;
        } catch {
            return false;
        }
    }

    /**
     * Attempt biometric authentication (fingerprint/face).
     * Returns 'success' on success.
     * Returns 'fallback' if not available or fallback requested.
     * Returns 'failed' if canceled.
     */
    async function authenticate(displayName: string): Promise<'success' | 'fallback' | 'failed'> {
        try {
            const available = await isBiometricAvailable();
            // If the device doesn't support biometrics, fallback to password
            if (!available) return 'fallback';

            await tauriAuthenticate(`请验证以切换至账户: ${displayName}`, {
                cancelTitle: '取消',
                fallbackTitle: '使用密码',
            });
            return 'success';
        } catch (err: any) {
            console.warn("Biometric auth error:", err);
            // On desktop or when user clicks "Use Password", fall back to manual password
            const eStr = String(err).toLowerCase();
            if (eStr.includes('fallback') || eStr.includes('not interactive') || eStr.includes('not supported')) {
                return 'fallback';
            }
            if (err && (err.errorCode === 'userFallback' || err.errorCode === 'biometryNotAvailable' || err.errorCode === 'notInteractive')) {
                return 'fallback';
            }
            return 'fallback'; // Defaulting to fallback is safer so users aren't locked out of accounts
        }
    }

    return { isBiometricAvailable, authenticate };
}
