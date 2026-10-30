#!/usr/bin/env node

// Warp v2.0 — Terminal Cognitive System
// POLER[n] + локальні моделі + реальний L-модуль

const fs = require('fs');
const path = require('path');
const readline = require('readline');
const blessed = require('blessed');
const contrib = require('blessed-contrib');

// ============================================
// ФІЗИЧНЕ ЯДРО POLER[n] (Обновлене)
// ============================================

class TensorPhysics {
constructor(dim = 10) {
this.dim = dim;
this.p = this.randomVector(dim); // Імпульс
this.J = this.antiSymmetricMatrix(dim);
this.D = this.dissipationMatrix(dim);
this.Lambda = this.laplacianMatrix(dim);
this.Sigma = 0.0;
this.A_t = 0.0;
this.history = [];

this.ALPHA = 0.5;
this.BETA = 0.8;
this.DELTA = 0.3;
this.KAPPA_E = 0.1;
this.GAMMA = 0.01;
this.MU = 0.05;
this.RHO = 0.95;
this.DT = 0.1;
}

randomVector(dim) {
return Array.from({length: dim}, () => Math.random());
}

antiSymmetricMatrix(dim) {
const M = Array(dim).fill().map(() => Array(dim).fill(0));
for (let i = 0; i < dim; i++) {
for (let j = i + 1; j < dim; j++) {
M[i][j] = Math.random() - 0.5;
M[j][i] = -M[i][j];
}
}
return M;
}

dissipationMatrix(dim) {
const M = Array(dim).fill().map(() => Array(dim).fill(0));
for (let i = 0; i < dim; i++) M[i][i] = 0.1;
return M;
}

laplacianMatrix(dim) {
const M = Array(dim).fill().map(() => Array(dim).fill(0));
for (let i = 0; i < dim; i++) M[i][i] = 1.0;
return M;
}

norm(v) {
return Math.sqrt(v.reduce((sum, val) => sum + val * val, 0));
}

matrixMultiply(A, B) {
const result = Array(A.length).fill().map(() => Array(B[0].length).fill(0));
for (let i = 0; i < A.length; i++) {
for (let j = 0; j < B[0].length; j++) {
for (let k = 0; k < A[0].length; k++) {
result[i][j] += A[i][k] * B[k][j];
}
}
}
return result;
}

matVecMult(M, V) {
return M.map(row => row.reduce((sum, val, idx) => sum + val * V[idx], 0));
}

step(input_vector, old_image_vector) {
// 1. СТАН
let p_t = [...this.p];

// 2. ТОПОЛОГІЯ
const S_I = this.norm(input_vector);
this.Sigma += S_I * this.A_t * this.DT;

// 3. АКТИВНІСТЬ
this.A_t = this.norm(input_vector);

// 4. ГРАДІЄНТИ
const DeltaOmega = input_vector.map((val, i) => val - (old_image_vector[i] || 0));
const DeltaOmega_sq = DeltaOmega.map(val => val * val);

let nabla_p_varepsilon_sum = DeltaOmega_sq.map(val => val * this.KAPPA_E);
let epsilon_list = [DeltaOmega_sq.reduce((sum, val) => sum + val, 0) * this.KAPPA_E];

// 5. РЕЗОНАНС P[n]
this.history.slice(-5).forEach(([p_hist, o_hist], k) => {
const rho_k = this.RHO ** (k + 1);
const D = o_hist.map((val, i) => input_vector[i] - val);
const D_sq = D.map(val => val * val);
const weighted = D_sq.map(val => val * this.KAPPA_E * this.GAMMA * rho_k);
nabla_p_varepsilon_sum = nabla_p_varepsilon_sum.map((val, i) => val + weighted[i]);
epsilon_list.push(D_sq.reduce((sum, val) => sum + val, 0) * this.KAPPA_E);
});

// 6. ОПЕРАТОР РУХУ
const S_p = this.J.map((row, i) =>
row.map((val, j) =>
val - this.D[i][j] - (this.MU * this.Sigma * this.Lambda[i][j])
)
);

// 7. ОНОВЛЕННЯ ІМПУЛЬСУ
const temp1 = this.matVecMult(this.Lambda, nabla_p_varepsilon_sum);
const temp2 = this.matVecMult(S_p, temp1);
const temp3 = this.matVecMult(this.Lambda, temp2);
const d_p = temp3.map(val => val * this.DT);
this.p = p_t.map((val, i) => val + d_p[i]);

// 8. МАСА
const mass_t = this.ALPHA * this.norm(input_vector) +
this.BETA * this.A_t +
this.DELTA * this.Sigma;

this.history.push([this.p, input_vector]);

return {
mass: mass_t,
sigma: this.Sigma,
force: this.norm(nabla_p_varepsilon_sum),
epsilonList: epsilon_list
};
}
}

// ============================================
// ЛОКАЛЬНИЙ МОДЕЛЬНИЙ ДВИГУН
// ============================================

class LocalModelEngine {
constructor() {
this.models = {
// Прості статичні моделі (можна замінити на реальні)
'logic-simple': {
analyze: (text) => ({
complexity: Math.min(text.length / 100, 1.0),
sentiment: Math.random() * 2 - 1,
keywords: text.split(' ').slice(0, 3)
})
},
'semantic-basic': {
embed: (text) => {
const hash = text.split('').reduce((a, b) => a + b.charCodeAt(0), 0);
return Array(10).fill().map((_, i) =>
Math.sin(hash + i * 0.1) * 0.5 + 0.5
);
}
}
};

this.currentModel = 'logic-simple';
}

setModel(name) {
if (this.models[name]) {
this.currentModel = name;
return true;
}
return false;
}

analyze(text) {
const model = this.models[this.currentModel];
if (!model) return null;

return model.analyze(text);
}

embed(text) {
const model = this.models['semantic-basic'];
if (!model) return Array(10).fill(0);

return model.embed(text);
}

// Можна додати завантаження реальних моделей
async loadExternalModel(modelPath) {
try {
// Місце для інтеграції з реальними моделями
            console.log(`[INFO] Завантаження моделі з ${modelPath}`);
return true;
} catch (error) {
            console.log(`[ERROR] Помилка завантаження: ${error.message}`);
return false;
}
}
}

// ============================================
// WARP v2.0 — ГОЛОВНА СИСТЕМА
// ============================================

class WarpSystem {
constructor() {
this.physics = new TensorPhysics(10);
this.localModel = new LocalModelEngine();
this.cycleCount = 0;
this.resonanceMemory = 0;
this.logs = [];
this.parameters = {
tau: 2.0,
kappa: 1.0,
n: 3
};

// Історія для графіків
this.history = {
mass: [],
sigma: [],
force: [],
time: []
};
}

log(message, level = 'INFO') {
const timestamp = new Date().toLocaleTimeString();
const entry = `[${timestamp}] ${level}: ${message}`;
this.logs.push(entry);
if (this.logs.length > 100) this.logs.shift();
return entry;
}

async runCycle(inputText) {
this.cycleCount++;

// ℘: Перцепція
const perception = this.localModel.embed(inputText);
const analysis = this.localModel.analyze(inputText);

this.log(`Цикл #${this.cycleCount}: "${inputText.substring(0, 30)}..."`);
        this.log(`Аналіз: складність=${analysis?.complexity?.toFixed(2)}`);

// O: Образ
const oldImage = this.physics.history.length > 0
? this.physics.history[this.physics.history.length - 1][1]
: Array(10).fill(0.5);

// L → ε → R[n]: Фізичне ядро
const result = this.physics.step(perception, oldImage);

// Оновлення резонансу
this.resonanceMemory = result.sigma;

// Збереження історії
this.history.mass.push(result.mass);
this.history.sigma.push(result.sigma);
this.history.force.push(result.force);
this.history.time.push(this.cycleCount);

// Обрізання історії
const maxHistory = 50;
if (this.history.mass.length > maxHistory) {
this.history.mass.shift();
this.history.sigma.shift();
this.history.force.shift();
this.history.time.shift();
}

return {
cycle: this.cycleCount,
mass: result.mass,
sigma: result.sigma,
force: result.force,
resonance: this.resonanceMemory,
analysis: analysis,
perception: perception
};
}

reset() {
this.physics = new TensorPhysics(10);
this.cycleCount = 0;
this.resonanceMemory = 0;
this.history = { mass: [], sigma: [], force: [], time: [] };
        this.log('Система скинута до початкового стану');
}

setParameter(param, value) {
if (this.parameters.hasOwnProperty(param)) {
this.parameters[param] = value;
            this.log(`Параметр ${param} встановлено на ${value}`);
return true;
}
return false;
}

getStatus() {
return {
cycle: this.cycleCount,
resonance: this.resonanceMemory,
parameters: this.parameters,
model: this.localModel.currentModel
};
}
}

// ============================================
// TUI (Terminal User Interface)
// ============================================

class WarpTUI {
constructor() {
this.system = new WarpSystem();
this.screen = blessed.screen({
smartCSR: true,
title: 'Warp v2.0 — Terminal Cognitive System'
});

this.initUI();
this.bindEvents();
this.updateDisplay();
}

initUI() {
// Головний лейаут
this.layout = blessed.layout({
parent: this.screen,
width: '100%',
height: '100%',
layout: 'grid'
});

// Панель статусу
this.statusBox = blessed.box({
parent: this.layout,
top: 0,
left: 0,
width: '50%',
height: '20%',
border: { type: 'line' },
style: { border: { fg: 'cyan' } },
            label: ' Статус '
});

this.statusText = blessed.text({
parent: this.statusBox,
top: 0,
left: 1,
width: '100%-2',
height: '100%',
            content: 'Завантаження...'
});

// Панель результатів
this.resultsBox = blessed.box({
parent: this.layout,
top: 0,
left: '50%',
width: '50%',
height: '20%',
border: { type: 'line' },
style: { border: { fg: 'green' } },
            label: ' Результати '
});

this.resultsText = blessed.text({
parent: this.resultsBox,
top: 0,
left: 1,
width: '100%-2',
height: '100%',
content: ''
});

// Лог панель
this.logBox = blessed.box({
parent: this.layout,
top: '20%',
left: 0,
width: '100%',
height: '30%',
border: { type: 'line' },
style: { border: { fg: 'yellow' } },
            label: ' Лог '
});

this.logList = blessed.list({
parent: this.logBox,
top: 0,
left: 1,
width: '100%-2',
height: '100%-2',
style: {
selected: { bg: 'blue', fg: 'white' }
},
scrollable: true,
keys: true,
mouse: true
});

// Графіки
this.chartBox = blessed.box({
parent: this.layout,
top: '50%',
left: 0,
width: '100%',
height: '30%',
border: { type: 'line' },
style: { border: { fg: 'magenta' } },
            label: ' Динаміка '
});

// Введення
this.inputBox = blessed.textbox({
parent: this.layout,
top: '80%',
left: 0,
width: '100%',
height: '20%',
border: { type: 'line' },
style: { border: { fg: 'white' } },
            label: ' Введення (℘) ',
inputOnFocus: true,
keys: true,
mouse: true
});

// Статус бар
this.statusBar = blessed.box({
parent: this.screen,
bottom: 0,
left: 0,
width: '100%',
height: 1,
style: { bg: 'blue', fg: 'white' }
});

this.statusBarText = blessed.text({
parent: this.statusBar,
            content: 'Warp v2.0 | F1: Допомога | Ctrl+S: Статус | Ctrl+R: Скинути | Ctrl+Q: Вийти'
});

this.layout.grid.set(0, 0, 1, 2, this.statusBox);
this.layout.grid.set(0, 2, 1, 2, this.resultsBox);
this.layout.grid.set(1, 0, 3, 4, this.logBox);
this.layout.grid.set(4, 0, 3, 4, this.chartBox);
this.layout.grid.set(7, 0, 2, 4, this.inputBox);
}

bindEvents() {
// Глобальні клавіші
this.screen.key(['C-s'], () => this.showStatus());
this.screen.key(['C-r'], () => this.resetSystem());
this.screen.key(['C-q'], () => process.exit(0));
this.screen.key(['f1'], () => this.showHelp());
this.screen.key(['escape'], () => process.exit(0));

// Введення тексту
this.inputBox.on('submit', async (value) => {
if (value.trim()) {
this.inputBox.clearValue();
this.inputBox.focus();

// Запуск циклу
const result = await this.system.runCycle(value);

// Оновлення UI
this.updateDisplay(result);

// Логування
this.logList.addItem(`[${this.system.cycleCount}] Введено: ${value.substring(0, 30)}...`);
this.logList.select(this.logList.items.length - 1);
this.logList.scrollTo(this.logList.items.length - 1);
}
});

// Фокус на введення
this.screen.key(['tab'], () => {
this.inputBox.focus();
});
}

updateDisplay(result = null) {
// Статус
const status = this.system.getStatus();
this.statusText.setContent(
`Цикл: ${status.cycle}\n` +
`Резонанс: ${status.resonance.toExponential(4)}\n` +
            `Модель: ${status.model}\n` +
`τ: ${status.parameters.tau} κ: ${status.parameters.kappa} n: ${status.parameters.n}`
);

// Результати
if (result) {
this.resultsText.setContent(
`Маса: ${result.mass.toExponential(4)}\n` +
`Топологія: ${result.sigma.toExponential(4)}\n` +
`Сила: ${result.force.toExponential(4)}\n` +
`Складність: ${result.analysis?.complexity?.toFixed(4) || 'N/A'}`
);
}

// Статус бар
this.statusBarText.setContent(
`Warp v2.0 | Циклів: ${status.cycle} | Резонанс: ${status.resonance.toExponential(4)} | ` +
            `Модель: ${status.model} | [Tab: введення] [F1: допомога]`
);

this.screen.render();
}

showStatus() {
const status = this.system.getStatus();
const message = blessed.message({
parent: this.screen,
border: { type: 'line' },
height: 'shrink',
width: 'shrink',
top: 'center',
left: 'center',
            label: ' Статус системи ',
tags: true,
keys: true,
vi: true
});

message.display(
`{bold}Цикл:{/bold} ${status.cycle}\n` +
`{bold}Резонанс:{/bold} ${status.resonance.toExponential(4)}\n` +
`{bold}Модель:{/bold} ${status.model}\n` +
            `{bold}Параметри:{/bold}\n` +
`  τ (пам'ять): ${status.parameters.tau}\n` +
`  κ (енергія): ${status.parameters.kappa}\n` +
`  n (резонанс): ${status.parameters.n}\n\n` +
            `{bold}Історія:{/bold}\n` +
`  Маса: ${this.system.history.mass.length} точок\n` +
`  Топологія: ${this.system.history.sigma.length} точок\n` +
`  Сила: ${this.system.history.force.length} точок`,
0,
() => {}
);
}

showHelp() {
const help = blessed.message({
parent: this.screen,
border: { type: 'line' },
height: 'shrink',
width: 'shrink',
top: 'center',
left: 'center',
            label: ' Довідка ',
tags: true,
keys: true,
vi: true
});

help.display(
