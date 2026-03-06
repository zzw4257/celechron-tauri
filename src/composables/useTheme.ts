import { computed } from 'vue';
import { usePreferences, type ThemePalette } from './usePreferences';

export type ThemeType = ThemePalette;

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

const {
  themeMode,
  themePalette,
  glassEffect,
  setThemeMode,
  setThemePalette,
  setGlassEffect: updateGlassEffect,
} = usePreferences();

function applyTheme(themeId: ThemeType) {
  if (typeof document === 'undefined') {
    return;
  }

  const theme = THEMES[themeId];
  const isDarkMode = themeMode.value === 'dark';
  const blobs = isDarkMode ? theme.darkBlobs : theme.lightBlobs;
  const root = document.documentElement;
  root.style.setProperty('--blob-1', blobs[0]);
  root.style.setProperty('--blob-2', blobs[1]);
  root.style.setProperty('--blob-3', blobs[2]);
  root.style.setProperty('--blob-4', blobs[3]);
}

function applyMode() {
  if (typeof document === 'undefined') {
    return;
  }

  const root = document.documentElement;
  root.dataset.theme = themeMode.value;
  root.style.colorScheme = themeMode.value;
  applyTheme(themePalette.value);
}

export function useTheme() {
  function setTheme(themeId: ThemeType) {
    setThemePalette(themeId);
    applyTheme(themeId);
  }

  function toggleDarkMode(dark: boolean) {
    setThemeMode(dark ? 'dark' : 'light');
    applyMode();
  }

  function toggleLightMode(light: boolean) {
    toggleDarkMode(!light);
  }

  function setGlassEffect(effect: 'liquid' | 'frosted') {
    updateGlassEffect(effect);
  }

  applyMode();

  return {
    currentTheme: themePalette,
    isLightMode: computed(() => themeMode.value === 'light'),
    isDarkMode: computed(() => themeMode.value === 'dark'),
    glassEffect,
    THEMES,
    setTheme,
    toggleLightMode,
    toggleDarkMode,
    setGlassEffect,
    applyTheme,
    applyMode,
  };
}
