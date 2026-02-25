// FORCE CLEAR CACHES BEFORE VUE IMPORTS
// Bump version key to force-invalidate stale timetable caches
if (localStorage.getItem('celechron_app_v11') !== 'clean') {
    localStorage.removeItem('celechron_theme_mode');
    localStorage.removeItem('celechron_theme');
    localStorage.removeItem('celechron_glass_effect');
    localStorage.removeItem('semester_start_ms');
    localStorage.setItem('celechron_app_v11', 'clean');
    // Clear old version keys
    localStorage.removeItem('celechron_app_v10');
    localStorage.removeItem('celechron_cache_version');
}
