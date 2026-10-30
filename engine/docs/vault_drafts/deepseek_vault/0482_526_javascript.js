// ==UserScript==
// @name         NotebookLM Ultimate Toolkit
// @namespace    http://tampermonkey.net/
// @version      1.0
// @description  AI assistant, prompt manager, chat exporter for NotebookLM
// @author       YourName
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
// 1. КОНСТАНТЫ И НАСТРОЙКИ
// ============================================================
const VERSION = '1.0';
const API_URL = 'https://openrouter.ai/api/v1/chat/completions';
const SETTINGS_KEY = 'nlm_ultimate_settings';
const CONTEXT_KEY = 'nlm_ultimate_context';
const HISTORY_KEY = 'nlm_ultimate_history';
const PROMPTS_KEY = 'nlm_ultimate_prompts';
const TAGS_KEY = 'nlm_ultimate_tags';

// Модели (из AI v17)
    const AVAILABLE_MODELS = { ... }; // скопировать из AI v17

const DEFAULT_SETTINGS = {
apiKey: "",
model: "openai/gpt-4o-mini",
temperature: 0.7,
maxTokens: 4000,
        systemPrompt: "Ты AI-ассистент. Отвечай на языке вопроса.",
autoSave: true,
showNotifications: true,
// настройки для промптов
promptsEnabled: true,
enhanceWithAI: true,
};

// ============================================================
// 2. СОСТОЯНИЕ
// ============================================================
let Settings = { ...DEFAULT_SETTINGS };
let Context = { chatContent: '', chapters: {}, lastUpdate: null };
let ChatHistory = [];
let isProcessing = false;
let uiCreated = false;

// Данные для промптов
let PromptsLibrary = [];
let PromptTags = {};
let currentPromptTags = new Set();

// Данные экспорта
let ExtractedExportData = { messages: [], sources: [] };
let ImportedHistoryText = '';
let ImportedHistoryFileName = '';

// ============================================================
// 3. ВСПОМОГАТЕЛЬНЫЕ ФУНКЦИИ (из AI v17)
// ============================================================
function sleep(ms) { ... }
function formatBytes(bytes) { ... }
function getModelName(modelId) { ... }
function loadSettings() { ... }
function saveSettings() { ... }
function loadContext() { ... }
function saveContext() { ... }
function loadHistory() { ... }
function saveHistory() { ... }

// ============================================================
// 4. ФУНКЦИИ РАБОТЫ С ПРОМПТАМИ (адаптированные из My Prompt)
// ============================================================
async function loadPrompts() {
try {
const saved = await GM_getValue(PROMPTS_KEY, '[]');
PromptsLibrary = JSON.parse(saved);
} catch(e) { console.error(e); PromptsLibrary = []; }
}
async function savePrompts() {
await GM_setValue(PROMPTS_KEY, JSON.stringify(PromptsLibrary));
}
async function addPrompt(prompt) {
const newPrompt = {
id: Date.now() + '-' + Math.random().toString(36),
...prompt,
position: PromptsLibrary.length
};
PromptsLibrary.push(newPrompt);
await savePrompts();
return newPrompt;
}
async function updatePrompt(id, data) {
const index = PromptsLibrary.findIndex(p => p.id == id);
if (index !== -1) {
PromptsLibrary[index] = { ...PromptsLibrary[index], ...data };
await savePrompts();
}
}
async function deletePrompt(id) {
PromptsLibrary = PromptsLibrary.filter(p => p.id != id);
await savePrompts();
}
function parsePromptInternal(text) {
// Скопировать функцию parsePromptInternal из My Prompt (она большая)
// ...
}
function openPlaceholderModal(prompt) {
// Скопировать openPlaceholderModal из My Prompt
// ...
}
async function enhancePromptWithAI(text, onResult) {
// Используем Settings.apiKey и Settings.model
// Вызываем AI через GM_xmlhttpRequest, как в callAI, но с особым системным промптом
// ...
}

// ============================================================
// 5. ФУНКЦИИ AI АССИСТЕНТА (из AI v17)
// ============================================================
async function collectContext() { ... }
function indexChapters() { ... }
function findChapter(number) { ... }
function callAI(userMessage, specificContext = null) { ... }
function processMessage(text) { ... }

// ============================================================
// 6. ФУНКЦИИ ЭКСПОРТА/ИМПОРТА (из AI v17)
// ============================================================
async function extractAll() { ... }
function exportToJSON() { ... }
function exportToMarkdown() { ... }
function exportToHTML() { ... }
function copyToClipboard() { ... }
function importHistoryFromFile() { ... }

// ============================================================
// 7. ФУНКЦИИ НАВИГАЦИИ ПО СООБЩЕНИЯМ (из My Prompt, опционально)
// ============================================================
function scanMessages() { ... }
function scrollToMessage(index) { ... }
function navigateToMessage(direction) { ... }

// ============================================================
// 8. UI ФУНКЦИИ (главная панель)
// ============================================================
function addMessage(text, type = 'ai') { ... }
function updateProgress(percent) { ... }
function updateStatus(status) { ... }
function updateContextBar() { ... }
function setButtonsDisabled(disabled) { ... }

function createPromptsTab() {
// Создаём HTML для вкладки "Промпты"
// Добавляем список промптов, кнопки "Новый", "Импорт", "Экспорт", поле поиска, фильтр по тегам
// Используем функции рендеринга
}

function renderPromptsList() {
// Отображает PromptsLibrary в контейнере
// Для каждого промпта: кнопки "Вставить", "Редактировать", "Удалить", "Улучшить"
}

function openPromptModal(prompt = null) {
// Модальное окно для создания/редактирования промпта (скопировано из My Prompt)
// ...
}

function insertPrompt(prompt, autoExecute = false) {
// Вставить текст промпта в поле ввода NotebookLM
// Если usePlaceholders == true, вызываем openPlaceholderModal
// ...
}

// ============================================================
// 9. СОЗДАНИЕ UI (главное меню)
// ============================================================
function createUI() {
if (uiCreated) return;
uiCreated = true;

// Загрузка данных
loadSettings();
loadContext();
loadHistory();
loadPrompts();

// Создаём плавающую кнопку и панель (как в AI v17)
const mainBtn = document.createElement('button');
mainBtn.id = 'nlm-main-btn';
mainBtn.innerHTML = '🤖';
mainBtn.title = 'NotebookLM Ultimate Toolkit';

const panel = document.createElement('div');
panel.id = 'nlm-panel';
// Вставляем HTML панели с вкладками: Чат, Настройки, Помощь, Промпты
panel.innerHTML = `
<div class="nlm-header">...</div>
<div class="nlm-tabs-container">
<div class="nlm-tabs">
<button class="nlm-tab active" data-tab="chat">💬 Чат</button>
<button class="nlm-tab" data-tab="prompts">📋 Промпты</button>
<button class="nlm-tab" data-tab="settings">⚙️ Настройки</button>
<button class="nlm-tab" data-tab="help">❓ Помощь</button>
</div>
</div>
<div class="nlm-context-bar">...</div>
<div class="nlm-tab-content active" id="tab-chat">...</div>
<div class="nlm-tab-content" id="tab-prompts">
<div class="nlm-prompts-container">
<div class="mp-search-bar">
<input type="text" id="mp-search-input" placeholder="Поиск промптов...">
<button id="mp-new-prompt">➕ Новый</button>
</div>
<div id="mp-prompts-list" class="mp-prompts-list"></div>
</div>
</div>
<div class="nlm-tab-content" id="tab-settings">...</div>
<div class="nlm-tab-content" id="tab-help">...</div>
`;
document.body.appendChild(mainBtn);
document.body.appendChild(panel);

// Обработчики вкладок
document.querySelectorAll('.nlm-tab').forEach(tab => {
tab.onclick = () => {
document.querySelectorAll('.nlm-tab').forEach(t => t.classList.remove('active'));
document.querySelectorAll('.nlm-tab-content').forEach(c => c.classList.remove('active'));
tab.classList.add('active');
document.getElementById(`tab-${tab.dataset.tab}`).classList.add('active');
if (tab.dataset.tab === 'prompts') renderPromptsList();
};
});

// Обработчики для чата (как в AI v17)
document.getElementById('nlm-send').onclick = sendMessage;
document.getElementById('nlm-input').onkeypress = e => { if (e.key === 'Enter') sendMessage(); };
document.getElementById('nlm-collect').onclick = collectContext;
document.getElementById('nlm-chapters').onclick = () => { ... };
document.getElementById('nlm-clear').onclick = () => { ... };
document.getElementById('nlm-export').onclick = async () => { ... };
document.getElementById('nlm-import').onclick = importHistoryFromFile;

// Обработчики для промптов
document.getElementById('mp-new-prompt').onclick = () => openPromptModal();
document.getElementById('mp-search-input').oninput = (e) => filterPrompts(e.target.value);

// Обработчики настроек (из AI v17)
// ...

// Дополнительно: регистрация команд в меню Tampermonkey
        GM_registerMenuCommand('🤖 Открыть панель', () => {
const panel = document.getElementById('nlm-panel');
if (panel) panel.classList.toggle('open');
});
}

function sendMessage() { ... }

// ============================================================
// 10. ИНИЦИАЛИЗАЦИЯ
// ============================================================
function init() {
loadSettings();
setTimeout(createUI, 2000);
}

init();
})();
