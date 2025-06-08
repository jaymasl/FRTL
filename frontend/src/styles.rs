pub const CONTAINER: &str = "min-h-screen bg-gray-50 dark:bg-gray-900 w-full px-4 sm:px-6 lg:px-8";
pub const CONTAINER_SM: &str = "max-w-md mx-auto px-4 sm:px-6 py-4 bg-gray-50 dark:bg-gray-900";
pub const CONTAINER_LG: &str = "max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-6 bg-gray-50 dark:bg-gray-900";
pub const NAV: &str = "fixed top-0 z-50 w-full bg-white/60 dark:bg-gray-700/60 backdrop-blur-md border-b border-gray-200/50 dark:border-gray-700/50";
pub const NAV_INNER: &str = "w-full h-16 px-4 sm:px-6 lg:px-8";
pub const NAV_CONTENT: &str = "h-full flex items-center justify-between";
pub const NAV_BRAND: &str = "flex items-center text-xl font-bold text-gray-900 dark:text-white hover:text-blue-600 dark:hover:text-blue-400 transition-colors duration-200";
pub const NAV_ITEMS: &str = "flex items-center space-x-4";
pub const NAV_LINK: &str = "relative px-3 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:text-blue-600 dark:hover:text-blue-400 transition-all duration-200 after:absolute after:left-0 after:bottom-0 after:h-0.5 after:w-full after:origin-right after:scale-x-0 after:bg-blue-600 dark:after:bg-blue-400 after:transition-transform hover:after:origin-left hover:after:scale-x-100";
pub const BUTTON_ICON: &str = "p-2 text-gray-800 dark:text-white hover:text-blue-600 dark:hover:text-blue-400 rounded-lg transition-colors duration-200";
pub const CARD: &str = "bg-white dark:bg-gray-800 rounded-lg shadow-lg dark:shadow-[0_4px_12px_-4px_rgba(255,255,255,0.03)] p-6";
pub const CARD_HOVER: &str = "bg-white dark:bg-gray-800 rounded-lg shadow-lg dark:shadow-[0_4px_12px_-4px_rgba(255,255,255,0.03)] hover:shadow-xl dark:hover:shadow-[0_6px_16px_-6px_rgba(255,255,255,0.06)] p-6 transform hover:-translate-y-1 transition-all duration-300 cursor-pointer";
pub const CARD_ERROR: &str = "bg-red-50 dark:bg-red-900/50 border border-red-200 dark:border-red-800 rounded-lg p-4 text-red-700 dark:text-red-200";
pub const CARD_SUCCESS: &str = "bg-green-50 dark:bg-green-900/50 border border-green-200 dark:border-green-800 rounded-lg p-4 text-green-700 dark:text-green-200";
pub const BUTTON_PRIMARY: &str = "inline-flex items-center justify-center px-4 py-2 rounded-lg font-medium text-white bg-gradient-to-r from-blue-600 to-blue-700 hover:from-blue-700 hover:to-blue-800 shadow-lg dark:shadow-[0_4px_12px_-4px_rgba(255,255,255,0.03)] hover:shadow-xl dark:hover:shadow-[0_6px_16px_-6px_rgba(255,255,255,0.06)] transition-all duration-300";
pub const BUTTON_BASE: &str = "inline-flex items-center justify-center transition-all duration-300";
pub const BUTTON_SECONDARY: &str = "inline-flex items-center justify-center px-4 py-2 rounded-lg font-medium border border-gray-300 dark:border-gray-600 text-gray-900 dark:text-white hover:bg-gray-50 dark:hover:bg-gray-800";
pub const BUTTON_DANGER: &str = "inline-flex items-center justify-center rounded-lg bg-red-600 px-4 py-2 font-medium text-white hover:bg-red-700";
pub const INPUT: &str = "mt-2 block w-full rounded-lg border-0 bg-white dark:bg-gray-900 py-2 px-3 text-gray-900 dark:text-white shadow-sm ring-1 ring-inset ring-gray-300 dark:ring-gray-700 placeholder:text-gray-400 focus:ring-2 focus:ring-blue-600";
pub const INPUT_ERROR: &str = "mt-2 block w-full rounded-lg border-0 bg-white dark:bg-gray-900 py-2 px-3 text-gray-900 dark:text-white shadow-sm ring-2 ring-inset ring-red-500 focus:ring-2 focus:ring-inset focus:ring-red-500 sm:text-sm";
pub const FORM: &str = "mt-4 space-y-4";
pub const TEXT_H1: &str = "text-3xl font-bold text-gray-900 dark:text-white";
pub const TEXT_H2: &str = "text-2xl font-bold text-gray-900 dark:text-white";
pub const TEXT_H3: &str = "text-xl font-bold text-gray-900 dark:text-white";
pub const TEXT_BODY: &str = "text-gray-600 dark:text-gray-300";
pub const TEXT_SMALL: &str = "text-sm text-gray-500 dark:text-gray-400";
pub const TEXT_ERROR: &str = "text-sm text-red-500 dark:text-red-400";
pub const TEXT_SUCCESS: &str = "text-sm text-green-500 dark:text-green-400";
pub const TEXT_LINK: &str = "text-blue-500 hover:text-blue-400 transition-colors";
pub const TEXT_LABEL: &str = "block text-sm font-medium text-gray-900 dark:text-white";
pub const TEXT_SECONDARY: &str = "text-gray-600 dark:text-gray-400";
pub const TEXT_HINT: &str = "text-xs text-gray-500 dark:text-gray-400 mt-1";
pub const LINK: &str = "text-blue-600 dark:text-blue-400 hover:text-blue-700 dark:hover:text-blue-300 transition-colors duration-200";
pub const AUTH_CARD: &str = "bg-white dark:bg-gray-900 rounded-xl shadow-xl dark:shadow-[0_6px_20px_-6px_rgba(255,255,255,0.04)] p-8 max-w-md w-full mx-auto backdrop-blur-lg bg-white/80 dark:bg-gray-900/80 border border-gray-200/50 dark:border-gray-700/50";
pub const AUTH_BUTTON: &str = "w-full py-3 px-4 text-sm font-semibold text-white bg-gradient-to-r from-blue-600 to-blue-700 hover:from-blue-700 hover:to-blue-800 rounded-lg transition-all duration-200 transform hover:translate-y-[-1px] hover:shadow-lg dark:hover:shadow-[0_4px_12px_-4px_rgba(255,255,255,0.05)] focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 dark:focus:ring-offset-gray-900";
pub const CARD_DASHBOARD: &str = "bg-white dark:bg-gray-900 rounded-lg shadow-lg dark:shadow-[0_4px_12px_-4px_rgba(255,255,255,0.03)] p-6 transition-all duration-300";
pub const ICON_WRAPPER_BLUE: &str = "flex items-center justify-center w-10 h-10 rounded-full bg-blue-100 dark:bg-blue-900";
pub const ICON_WRAPPER_PURPLE: &str = "flex items-center justify-center w-10 h-10 rounded-full bg-purple-100 dark:bg-purple-900";
pub const ICON_WRAPPER_GREEN: &str = "flex items-center justify-center w-10 h-10 rounded-full bg-green-100 dark:bg-green-900";
pub const ICON: &str = "w-5 h-5 text-current";
pub const SECTION_GRID: &str = "py-8 grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6";
pub const FLEX_CENTER: &str = "flex min-h-[80vh] flex-col justify-center px-6 py-12 lg:px-8";
pub const FORM_CONTAINER: &str = "w-full max-w-sm";
pub const ALERT_SUCCESS: &str = "bg-green-50 dark:bg-green-900/50 border border-green-200 dark:border-green-800 rounded-lg p-4 text-green-700 dark:text-green-200";
pub const ALERT_ERROR: &str = "bg-red-50 dark:bg-red-900/50 border border-red-200 dark:border-red-800 rounded-lg p-4 text-red-700 dark:text-red-200";
pub const ALERT_WARNING: &str = "p-4 mb-4 text-sm text-yellow-800 rounded-lg bg-yellow-50 dark:bg-gray-900 dark:text-yellow-400";
pub const AUTH_HEADER: &str = "mb-6 text-center";
pub const VALIDATION_LIST: &str = "mt-2 space-y-1";
pub const CARD_TITLE: &str = "text-lg font-semibold text-gray-900 dark:text-white";
pub const CARD_TEXT: &str = "text-sm text-gray-600 dark:text-gray-400";
pub const LOADING_SPINNER: &str = "animate-spin h-5 w-5 text-blue-600 dark:text-blue-400";
pub const FOOTER: &str = "w-full bg-white/80 dark:bg-gray-900/80 backdrop-blur-md border-t border-gray-200/50 dark:border-gray-700/50";
pub const FOOTER_LINK: &str = "text-sm font-medium text-gray-700 dark:text-gray-300 hover:text-blue-600 dark:hover:text-blue-400 transition-colors duration-200";
pub const DROPDOWN: &str = "absolute left-1/2 -translate-x-1/2 bg-white dark:bg-gray-800 rounded-lg shadow-lg dark:shadow-[0_4px_12px_-4px_rgba(255,255,255,0.03)] ring-1 ring-black ring-opacity-5 focus:outline-none divide-y divide-gray-100 dark:divide-gray-700";
pub const DROPDOWN_BUTTON: &str = "w-full px-2 py-3 text-sm text-center text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 hover:text-blue-600 dark:hover:text-blue-400 transition-colors duration-200 first:rounded-t-lg last:rounded-b-lg";

// Hero section
pub const HERO_CONTAINER: &str = "relative flex items-center justify-center";
pub const HERO_CONTENT: &str = "max-w-3xl mx-auto px-4 sm:px-6 lg:px-8 py-16 text-center";
pub const HERO_TITLE: &str = "text-7xl font-bold text-transparent bg-clip-text bg-gradient-to-r from-blue-400 to-purple-400 mb-6";
pub const HERO_SUBTITLE: &str = "text-3xl font-medium text-gray-200 mb-4";
pub const HERO_DESCRIPTION: &str = "text-xl text-gray-300 max-w-2xl mx-auto";
pub const HERO_SECTION: &str = "flex flex-col items-center justify-center min-h-[calc(100vh-3rem)] py-12 px-4";
pub const HERO_TEXT: &str = "text-xl text-gray-600 dark:text-gray-300 max-w-2xl mx-auto";
pub const HERO_BUTTONS: &str = "flex justify-center items-center gap-4";
pub const HERO_FEATURES: &str = "p-8 rounded-2xl bg-white/5 backdrop-blur-lg border border-white/10 hover:border-white/20 transition-all duration-300";
pub const HERO_FEATURES_TITLE: &str = "text-2xl font-semibold mb-6 text-gray-100";
pub const HERO_FEATURES_LIST: &str = "space-y-4 text-lg text-gray-300";
pub const HERO_CTA_BUTTON: &str = "px-8 py-4 text-lg font-semibold text-white bg-gradient-to-r from-blue-500 to-purple-500 rounded-xl hover:from-blue-600 hover:to-purple-600 transform hover:scale-105 transition-all duration-300 shadow-lg hover:shadow-xl";
pub const HERO_LOGIN_BUTTON: &str = "text-gray-400 hover:text-gray-200 transition-colors duration-200";

pub const FEATURE_CARD: &str = "flex items-start space-x-4 p-4 rounded-2xl bg-gradient-to-br from-blue-100 via-blue-200 to-blue-100 dark:from-blue-800/20 dark:via-blue-900/30 dark:to-blue-800/20 border border-blue-200 dark:border-blue-800";
pub const FEATURE_CARD_PURPLE: &str = "flex items-start space-x-4 p-4 rounded-2xl bg-gradient-to-br from-purple-50 via-purple-100 to-purple-50 dark:from-purple-900/20 dark:via-purple-900/30 dark:to-purple-900/20 border border-purple-100 dark:border-purple-800";
pub const FEATURE_CARD_INDIGO: &str = "flex items-start space-x-4 p-4 rounded-2xl bg-gradient-to-br from-indigo-200 via-indigo-300 to-indigo-200 dark:from-indigo-900/30 dark:via-indigo-950/40 dark:to-indigo-900/30 border border-indigo-300 dark:border-indigo-800";
pub const FEATURE_ITEM: &str = "flex items-start space-x-4 p-4 rounded-2xl bg-blue-900/20 border border-blue-800";
pub const FEATURE_GRID: &str = "grid grid-cols-1 sm:grid-cols-3 gap-6";

// Title section
pub const HERO_TITLE_CONTAINER: &str = "mx-auto max-w-4xl py-32";
pub const HERO_TITLE_WRAPPER: &str = "text-center";
pub const TITLE_GLOW: &str = "relative inline-flex group";
pub const TITLE_TEXT: &str = "relative px-7 py-4 text-7xl font-black tracking-tight text-transparent bg-clip-text bg-gradient-to-r from-purple-500 to-blue-600";
pub const SUBTITLE_TEXT: &str = "mt-6 text-2xl leading-9 tracking-tight bg-clip-text text-transparent bg-gradient-to-r from-gray-600 to-gray-900 dark:from-gray-300 dark:to-white";
pub const DESCRIPTION_TEXT: &str = "mt-6 text-lg leading-8 text-gray-600 dark:text-gray-300";

// Call to action
pub const CTA_CONTAINER: &str = "mt-10 flex flex-col sm:flex-row items-center justify-center gap-6";
pub const CTA_BUTTON_WRAPPER: &str = "relative inline-flex group";
pub const CTA_BUTTON_GLOW: &str = "absolute -inset-0.5 bg-gradient-to-r from-purple-600 to-blue-600 rounded-lg blur opacity-50 group-hover:opacity-75 transition duration-1000";
pub const CTA_BUTTON: &str = "relative px-8 py-4 bg-black rounded-lg leading-none flex items-center";
pub const CTA_BUTTON_TEXT: &str = "text-gray-100 group-hover:text-white duration-200";
pub const CTA_BUTTON_ICON: &str = "flex items-center justify-center w-8 h-8 -mr-2 ml-2 rounded-full bg-gradient-to-r from-purple-500/90 to-blue-600/90 group-hover:translate-x-1 transition-all";
pub const LOGIN_BUTTON: &str = "group rounded-full px-8 py-4 text-sm font-semibold text-gray-900 dark:text-white hover:bg-gray-100/90 dark:hover:bg-gray-800/90 transition";
pub const LOGIN_BUTTON_TEXT: &str = "ml-2 text-blue-600 group-hover:text-blue-500 dark:text-blue-400 dark:group-hover:text-blue-300";

// Focus view styles
pub const FOCUS_GRID: &str = "grid grid-cols-1 md:grid-cols-[3.3fr_1fr] gap-4";
pub const FOCUS_IMAGE_CONTAINER: &str = "relative rounded-2xl overflow-hidden bg-gradient-to-br from-gray-50 to-gray-100 dark:from-gray-800 dark:to-gray-900 ring-1 ring-gray-200 dark:ring-white/10 shadow-2xl dark:shadow-[0_8px_24px_-8px_rgba(255,255,255,0.04)] min-h-[540px] flex items-center justify-center";
pub const FOCUS_CARD: &str = "bg-white dark:bg-gray-800 rounded-2xl p-6 ring-1 ring-gray-200 dark:ring-white/10 shadow-lg dark:shadow-[0_4px_12px_-4px_rgba(255,255,255,0.03)]";
pub const FOCUS_CARD_TITLE: &str = "text-lg font-medium text-gray-900 dark:text-white mb-4";
pub const FOCUS_GRID_CONTENT: &str = "grid grid-cols-1 gap-4";
pub const FOCUS_LABEL: &str = "text-sm font-medium text-gray-600 dark:text-gray-400";
pub const FOCUS_VALUE: &str = "text-sm text-gray-900 dark:text-white";
pub const FOCUS_VALUE_SECONDARY: &str = "text-sm text-gray-500 dark:text-gray-400";
pub const FOCUS_GROUP: &str = "space-y-1";
pub const FOCUS_TITLE: &str = "text-2xl font-bold text-gray-900 dark:text-white";
pub const FOCUS_BUTTON: &str = "px-6 py-3 rounded-xl font-semibold shadow-lg dark:shadow-[0_4px_12px_-4px_rgba(255,255,255,0.03)] transition-all duration-500 ease-in-out";

// Animation classes
pub const ANIMATE_FLOAT_UP: &str = "animate-float-up";
pub const ANIMATE_FADE_IN: &str = "animate-fade-in";

pub const TAILWIND_CSS: &str = r#"
@tailwind base;
@tailwind components;
@tailwind utilities;

@keyframes chaosRealm {
    0% { width: 0; }
    100% { width: 100%; }
}

@keyframes fadeIn {
    0% { opacity: 0; }
    100% { opacity: 1; }
}

.animate-chaos-pulse {
    animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
}

.animate-fadeIn {
    animation: fadeIn 0.5s ease-in-out;
}

/* Custom scrollbar for webkit browsers */
::-webkit-scrollbar {
    width: 8px;
    height: 8px;
}

::-webkit-scrollbar-track {
    background: rgba(0, 0, 0, 0.1);
    border-radius: 4px;
}

::-webkit-scrollbar-thumb {
    background: rgba(128, 128, 128, 0.5);
    border-radius: 4px;
}

::-webkit-scrollbar-thumb:hover {
    background: rgba(128, 128, 128, 0.7);
}
"#;