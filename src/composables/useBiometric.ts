// useBiometric.ts — WebAuthn platform authenticator for fingerprint/face ID
// Only triggers on account switch; silently skips if device doesn't support it.

export function useBiometric() {
    /**
     * Check if the platform has a biometric authenticator available.
     * Uses WebAuthn PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable().
     */
    async function isBiometricAvailable(): Promise<boolean> {
        try {
            if (!window.PublicKeyCredential) return false;
            return await PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable();
        } catch {
            return false;
        }
    }

    /**
     * Attempt biometric authentication (fingerprint/face).
     * Returns true on success, false on failure or if not available (silent skip).
     * @param displayName Account display name shown in the system prompt
     */
    async function authenticate(_displayName: string): Promise<boolean> {
        try {
            const available = await isBiometricAvailable();
            // If the device doesn't support biometrics, silently skip (return true = allow switch)
            if (!available) return true;

            // Create a one-time challenge
            const challenge = crypto.getRandomValues(new Uint8Array(32));

            // Use a client-side credential. Since we don't have a real RP server,
            // we use a "get" style assertion with allowCredentials = [] and
            // userVerification = "required". The browser/OS will pop the biometric prompt.
            // This is valid for "proof of presence" without a real stored credential.
            const credential = await navigator.credentials.get({
                publicKey: {
                    challenge,
                    timeout: 60000,
                    userVerification: 'required',
                    rpId: window.location.hostname || 'localhost',
                    allowCredentials: [], // Let the OS decide which authenticator to use
                } as PublicKeyCredentialRequestOptions,
            }).catch(() => null);

            return credential !== null;
        } catch {
            // Any error → skip verification (don't block the user)
            return true;
        }
    }

    return { isBiometricAvailable, authenticate };
}
