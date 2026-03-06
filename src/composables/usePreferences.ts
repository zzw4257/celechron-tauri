import { computed, ref } from 'vue';

export type RetakePolicy = 'first' | 'highest';
export type ThemeMode = 'light' | 'dark';
export type ThemePalette = 'monterey' | 'midnight' | 'aurora' | 'crimson';
export type GlassEffect = 'liquid' | 'frosted';
export type TimeConfigMode = 'remote' | 'manual';

export interface Preferences {
  retakePolicy: RetakePolicy;
  hideGpa: boolean;
  themeMode: ThemeMode;
  themePalette: ThemePalette;
  glassEffect: GlassEffect;
  timeConfigMode: TimeConfigMode;
  manualSemesterAnchors: Record<string, string>;
  courseIdMappings: Record<string, string>;
  dingtalkWebhookEnabled: boolean;
  zeroClawEndpoint: string;
}

const STORAGE_KEY = 'celechron_preferences_v2';

const DEFAULTS: Preferences = {
  retakePolicy: 'first',
  hideGpa: false,
  themeMode: 'light',
  themePalette: 'monterey',
  glassEffect: 'frosted',
  timeConfigMode: 'remote',
  manualSemesterAnchors: {},
  courseIdMappings: {},
  dingtalkWebhookEnabled: false,
  zeroClawEndpoint: '',
};

function sanitize(input: Partial<Preferences> | null | undefined): Preferences {
  const retakePolicy = input?.retakePolicy === 'highest' || (input as any)?.retakePolicy === 'best'
    ? 'highest'
    : 'first';
  const themeMode = input?.themeMode === 'dark' ? 'dark' : 'light';
  const themePalette = ['monterey', 'midnight', 'aurora', 'crimson'].includes(String(input?.themePalette))
    ? (input?.themePalette as ThemePalette)
    : DEFAULTS.themePalette;
  const glassEffect = input?.glassEffect === 'liquid' ? 'liquid' : 'frosted';
  const timeConfigMode = input?.timeConfigMode === 'manual' ? 'manual' : 'remote';

  return {
    ...DEFAULTS,
    ...input,
    retakePolicy,
    themeMode,
    themePalette,
    glassEffect,
    timeConfigMode,
    hideGpa: Boolean(input?.hideGpa),
    manualSemesterAnchors: input?.manualSemesterAnchors || {},
    courseIdMappings: input?.courseIdMappings || {},
    dingtalkWebhookEnabled: Boolean(input?.dingtalkWebhookEnabled),
    zeroClawEndpoint: String(input?.zeroClawEndpoint || ''),
  };
}

function loadPreferences(): Preferences {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      return sanitize(JSON.parse(raw));
    }
  } catch {}

  const legacyRetakePolicy = localStorage.getItem('retakePolicy');

  return sanitize({
    retakePolicy: legacyRetakePolicy === 'highest' || legacyRetakePolicy === 'best' ? 'highest' : 'first',
    hideGpa: localStorage.getItem('hideGpa') === 'true',
    themeMode: localStorage.getItem('celechron_theme_mode') === 'dark' ? 'dark' : 'light',
    themePalette: (localStorage.getItem('celechron_theme') as ThemePalette | null) || DEFAULTS.themePalette,
    glassEffect: (localStorage.getItem('celechron_glass_effect') as GlassEffect | null) || DEFAULTS.glassEffect,
  });
}

const preferencesState = ref<Preferences>(loadPreferences());
const accountScope = ref(0);

function persist() {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(preferencesState.value));
}

function patchPreferences(patch: Partial<Preferences>) {
  preferencesState.value = sanitize({
    ...preferencesState.value,
    ...patch,
  });
  persist();
}

function setManualSemesterAnchor(termName: string, date: string | null) {
  const next = {
    ...preferencesState.value.manualSemesterAnchors,
  };
  if (date) {
    next[termName] = date;
  } else {
    delete next[termName];
  }
  patchPreferences({ manualSemesterAnchors: next });
}

function bumpAccountScope() {
  accountScope.value += 1;
}

export function usePreferences() {
  const preferences = computed(() => preferencesState.value);
  const retakePolicy = computed(() => preferencesState.value.retakePolicy);
  const hideGpa = computed(() => preferencesState.value.hideGpa);
  const themeMode = computed(() => preferencesState.value.themeMode);
  const themePalette = computed(() => preferencesState.value.themePalette);
  const glassEffect = computed(() => preferencesState.value.glassEffect);
  const timeConfigMode = computed(() => preferencesState.value.timeConfigMode);
  const manualSemesterAnchors = computed(() => preferencesState.value.manualSemesterAnchors);
  const courseIdMappings = computed(() => preferencesState.value.courseIdMappings);
  const zeroClawEndpoint = computed(() => preferencesState.value.zeroClawEndpoint);

  return {
    preferences,
    accountScope,
    retakePolicy,
    hideGpa,
    themeMode,
    themePalette,
    glassEffect,
    timeConfigMode,
    manualSemesterAnchors,
    courseIdMappings,
    zeroClawEndpoint,
    patchPreferences,
    setManualSemesterAnchor,
    bumpAccountScope,
    setRetakePolicy: (value: RetakePolicy) => patchPreferences({ retakePolicy: value }),
    setHideGpa: (value: boolean) => patchPreferences({ hideGpa: value }),
    setThemeMode: (value: ThemeMode) => patchPreferences({ themeMode: value }),
    setThemePalette: (value: ThemePalette) => patchPreferences({ themePalette: value }),
    setGlassEffect: (value: GlassEffect) => patchPreferences({ glassEffect: value }),
    setTimeConfigMode: (value: TimeConfigMode) => patchPreferences({ timeConfigMode: value }),
    setCourseIdMappings: (value: Record<string, string>) => patchPreferences({ courseIdMappings: value }),
    setZeroClawEndpoint: (value: string) => patchPreferences({ zeroClawEndpoint: value.trim() }),
  };
}
