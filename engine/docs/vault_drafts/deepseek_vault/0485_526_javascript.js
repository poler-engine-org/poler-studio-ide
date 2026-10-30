// ==UserScript==
// @name         NotebookLM Ultimate (AI + Prompts + Export)
// @namespace    https://github.com/yourname
// @version      1.0
// @description  Professional tool for NotebookLM: AI assistant, prompt manager, history export/import
// @match        https://notebooklm.google.com/*
// @grant        GM_xmlhttpRequest
// @grant        GM_addStyle
// @grant        GM_setValue
// @grant        GM_getValue
// @grant        GM_registerMenuCommand
// @run-at       document-end
// ==/UserScript==

(function() {
'use strict';

// ============================================================
// КОНСТАНТЫ
// ============================================================
const SCRIPT_VERSION = '1.0';
const API_URL = 'https://openrouter.ai/api/v1/chat/completions';
const SETTINGS_KEY = 'nlm_ultimate_settings';
const CONTEXT_KEY = 'nlm_ultimate_context';
const HISTORY_KEY = 'nlm_ultimate_history';
const PROMPTS_KEY = 'nlm_ultimate_prompts';
const TAGS_KEY = 'nlm_ultimate_tags';
const SHORTCUTS_KEY = 'nlm_ultimate_shortcuts';
const NAV_KEY = 'nlm_ultimate_nav';

// Модели (из AI v16)
const AVAILABLE_MODELS = {
groups: [
            { name: "🔥 Бесплатные", models: [
{ id: "meta-llama/llama-3.3-70b-instruct:free", name: "Llama 3.3 70B", desc: "Мощная, бесплатная" },
{ id: "deepseek/deepseek-chat:free", name: "DeepSeek V3", desc: "Быстрая, бесплатная" },
{ id: "google/gemma-3-27b-it:free", name: "Gemma 3 27B", desc: "Google, бесплатно" }
] },
            { name: "⚡ Оптимальные", models: [
{ id: "openai/gpt-4o-mini", name: "GPT-4o Mini", desc: "Быстрая, дешёвая" },
{ id: "anthropic/claude-3.5-haiku", name: "Claude 3.5 Haiku", desc: "Быстрая, качественная" }
] },
            { name: "🧠 Премиум", models: [
{ id: "openai/gpt-4o", name: "GPT-4o", desc: "Топ от OpenAI" },
{ id: "anthropic/claude-3.5-sonnet", name: "Claude 3.5 Sonnet", desc: "Топ от Anthropic" }
] }
]
};

const DEFAULT_SETTINGS = {
apiKey: "",
model: "openai/gpt-4o-mini",
temperature: 0.7,
maxTokens: 4000,
        systemPrompt: "Ты AI-ассистент. Отвечай на языке вопроса.",
autoSave: true
};

const DEFAULT_TAGS = { tags: {}, activeFilters: [] };
const DEFAULT_NAV = { enabled: true, filterMode: "all" };
const DEFAULT_SHORTCUTS = {
