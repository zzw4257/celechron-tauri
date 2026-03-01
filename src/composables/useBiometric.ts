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

            await tauriAuthenticate(`验证身份以切换账户: ${displayName}`, {
                cancelTitle: '取消',
                fallbackTitle: '使用密码',
            });
            return 'success';
        } catch (err: any) {
            console.warn("Biometric auth error:", err);
            const eStr = String(err).toLowerCase();

            // Check for specific cancellation or failure codes
            if (eStr.includes('canceled') || eStr.includes('usercancel') || eStr.includes('cancel')) {
                return 'failed';
            }
            if (eStr.includes('locked') || eStr.includes('too many attempts')) {
                return 'failed'; // Or throw a specific error
            }

            // For other cases like 'not supported' or 'fallback requested', we return fallback
            if (eStr.includes('fallback') || eStr.includes('not interactive') || eStr.includes('not supported')) {
                return 'fallback';
            }

            return 'fallback';
        }
    }

    return { isBiometricAvailable, authenticate };
}
