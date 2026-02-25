import { ref, computed } from 'vue';

export type ThemeType = 'monterey' | 'midnight' | 'aurora' | 'crimson';

export const THEMES: Record<ThemeType, { name: string; lightBlobs: string[]; darkBlobs: string[] }> = {
    monterey: {
        name: 'macOS Monterey (Default)',
        lightBlobs: ['#7dd3fc', '#c4b5fd', '#93c5fd', '#f9a8d4'],
        darkBlobs: ['#0e7490', '#5b21b6', '#1d4ed8', '#9d174d'],
    },
    midnight: {
        name: 'Midnight Glass',
        lightBlobs: ['#818cf8', '#6366f1', '#a5b4fc', '#c7d2fe'],
        darkBlobs: ['#1e1b4b', '#312e81', '#1e1b4b', '#4c1d95'],
    },
    aurora: {
        name: 'Aurora Borealis',
        lightBlobs: ['#6ee7b7', '#5eead4', '#a7f3d0', '#93c5fd'],
        darkBlobs: ['#065f46', '#047857', '#0d9488', '#3b82f6'],
    },
    crimson: {
        name: 'Crimson Ember',
        lightBlobs: ['#fca5a5', '#fbcfe8', '#fcd34d', '#fdba74'],
        darkBlobs: ['#7f1d1d', '#991b1b', '#9d174d', '#ea580c'],
    },
};

const currentTheme = ref<ThemeType>((localStorage.getItem('celechron_theme') as ThemeType) || 'monterey');
// Default: light mode (isDark = false)
const isDarkMode = ref<boolean>(localStorage.getItem('celechron_theme_mode') === 'dark');
const glassEffect = ref<'liquid' | 'frosted'>((localStorage.getItem('celechron_glass_effect') as 'liquid' | 'frosted') || 'frosted');

export function useTheme() {
    function setTheme(t: ThemeType) {
        currentTheme.value = t;
        localStorage.setItem('celechron_theme', t);
        applyTheme(t);
    }

    function toggleDarkMode(dark: boolean) {
        isDarkMode.value = dark;
        localStorage.setItem('celechron_theme_mode', dark ? 'dark' : 'light');
        applyModeClass();
    }

    // legacy alias used by OptionView
    function toggleLightMode(light: boolean) {
        toggleDarkMode(!light);
    }

    function setGlassEffect(effect: 'liquid' | 'frosted') {
        glassEffect.value = effect;
        localStorage.setItem('celechron_glass_effect', effect);
    }

    function applyTheme(t: ThemeType) {
        const theme = THEMES[t];
        const blobs = isDarkMode.value ? theme.darkBlobs : theme.lightBlobs;
        document.documentElement.style.setProperty('--blob-1', blobs[0]);
        document.documentElement.style.setProperty('--blob-2', blobs[1]);
        document.documentElement.style.setProperty('--blob-3', blobs[2]);
        document.documentElement.style.setProperty('--blob-4', blobs[3]);
    }

    function applyModeClass() {
        if (isDarkMode.value) {
            document.documentElement.classList.add('dark-theme');
            document.documentElement.classList.remove('light-theme');
        } else {
            document.documentElement.classList.remove('dark-theme');
            document.documentElement.classList.add('light-theme');
        }
        // Re-apply blobs for correct color palette
        applyTheme(currentTheme.value);
    }

    // Init on first import
    applyModeClass();

    return {
        currentTheme,
        // BUG FIX: was ref(!isDarkMode.value) — a one-time snapshot that never updated.
        // Now computed() so it stays reactive when isDarkMode changes.
        isLightMode: computed(() => !isDarkMode.value),
        isDarkMode,
        glassEffect,
        THEMES,
        setTheme,
        toggleLightMode,
        toggleDarkMode,
        setGlassEffect,
        applyTheme,
    };
}
