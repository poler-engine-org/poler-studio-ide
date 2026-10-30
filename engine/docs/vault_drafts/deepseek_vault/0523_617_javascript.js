#!/usr/bin/env node

// Warp v2.1 — Terminal Cognitive System
// POLER[n] + локальные модели + реальный L-модуль + DYNAMIS

const fs = require('fs');
const path = require('path');
const readline = require('readline');
const blessed = require('blessed');

// ============================================
// ФИЗИЧЕСКОЕ ЯДРО POLER[n] (Улучшенное)
// ============================================

class TensorPhysics {
constructor(dim = 12) {
this.dim = dim;
this.p = this.randomVector(dim);
this.J = this.antiSymmetricMatrix(dim);
this.D = this.dissipationMatrix(dim);
this.Lambda = this.laplacianMatrix(dim);
this.Sigma = 0.0;
this.A_t = 0.0;
this.history = [];

// Параметры системы
this.ALPHA = 0.5;
this.BETA = 0.8;
this.DELTA = 0.3;
this.KAPPA_E = 0.15;
this.GAMMA = 0.02;
this.MU = 0.06;
this.RHO = 0.93;
this.DT = 0.15;
}

randomVector(dim) {
return Array.from({length: dim}, () => Math.random() * 2 - 1);
}

antiSymmetricMatrix(dim) {
const M = Array(dim).fill().map(() => Array(dim).fill(0));
for (let i = 0; i < dim; i++) {
for (let j = i + 1; j < dim; j++) {
const val = Math.random() * 2 - 1;
M[i][j] = val * 0.3;
M[j][i] = -M[i][j];
}
}
return M;
}

dissipationMatrix(dim) {
const M = Array(dim).fill().map(() => Array(dim).fill(0));
for (let i = 0; i < dim; i++) M[i][i] = 0.08;
return M;
}

laplacianMatrix(dim) {
const M = Array(dim).fill().map(() => Array(dim).fill(0));
for (let i = 0; i < dim; i++) M[i][i] = 1.2;
return M;
}

norm(v) {
return Math.sqrt(v.reduce((sum, val) => sum + val * val, 0));
}

matVecMult(M, V) {
return M.map(row => row.reduce((sum, val, idx) => sum + val * V[idx], 0));
}

step(input_vector, old_image_vector) {
// 1. Текущее состояние
let p_t = [...this.p];

// 2. Активность (входной потенциал)
this.A_t = this.norm(input_vector) * 0.8;

// 3. Топология (накопленная кривизна)
const S_I = this.norm(input_vector);
this.Sigma += S_I * this.A_t * this.DT * 0.5;

// 4. Энергетические градиенты
const DeltaOmega = input_vector.map((val, i) =>
val - (old_image_vector[i] || 0));
const DeltaOmega_sq = DeltaOmega.map(val => val * val);

let nabla_p_varepsilon_sum = DeltaOmega_sq.map(val =>
val * this.KAPPA_E);
let epsilon_list = [DeltaOmega_sq.reduce((sum, val) =>
sum + val, 0) * this.KAPPA_E];

// 5. Резонансная память P[n]
this.history.slice(-4).forEach(([p_hist, o_hist], k) => {
const rho_k = this.RHO ** (k + 1);
const D = o_hist.map((val, i) => input_vector[i] - val);
const D_sq = D.map(val => val * val);
const weighted = D_sq.map(val =>
val * this.KAPPA_E * this.GAMMA * rho_k);
nabla_p_varepsilon_sum = nabla_p_varepsilon_sum.map((val, i) =>
val + weighted[i]);
epsilon_list.push(D_sq.reduce((sum, val) =>
sum + val, 0) * this.KAPPA_E);
});

// 6. Оператор движения
const S_p = this.J.map((row, i) =>
row.map((val, j) =>
val - this.D[i][j] - (this.MU * this.Sigma * this.Lambda[i][j])
)
);

// 7. Обновление импульса p_{t+1}
const temp1 = this.matVecMult(this.Lambda, nabla_p_varepsilon_sum);
const temp2 = this.matVecMult(S_p, temp1);
const temp3 = this.matVecMult(this.Lambda, temp2);
const d_p = temp3.map(val => val * this.DT);
this.p = p_t.map((val, i) => val + d_p[i]);

// 8. Динамическая масса
const mass_t = this.ALPHA * this.norm(input_vector) +
this.BETA * this.A_t +
this.DELTA * this.Sigma;

// 9. Сохранение истории
this.history.push([this.p, input_vector]);
if (this.history.length > 20) this.history.shift();

return {
mass: mass_t,
sigma: this.Sigma,
force: this.norm(nabla_p_varepsilon_sum),
epsilonList: epsilon_list,
activity: this.A_t
};
}
}

// ============================================
// ЛОКАЛЬНЫЙ МОДЕЛЬНЫЙ ДВИГАТЕЛЬ
// ============================================

class LocalModelEngine {
constructor() {
this.models = {
'logic-simple': {
analyze: (text) => {
const words = text.split(/\s+/).length;
const chars = text.length;
const avgWordLength = chars / Math.max(words, 1);

return {
complexity: Math.min(words / 50 + avgWordLength / 10, 1.0),
sentiment: Math.sin(text.length * 0.1) * 0.7,
keywords: text.split(/\s+/).slice(0, 3),
wordCount: words
};
}
},
'semantic-advanced': {
embed: (text) => {
// Более сложное преобразование текста в вектор
const chars = text.toLowerCase().replace(/[^a-zа-яё0-9]/g, '');
const vector = Array(12).fill(0);

// Распределяем символы по измерениям
for (let i = 0; i < chars.length; i++) {
const idx = i % 12;
vector[idx] += chars.charCodeAt(i) * 0.001;
}

// Нормализация
const norm = Math.sqrt(vector.reduce((s, v) => s + v*v, 0)) || 1;
return vector.map(v => v / norm);
}
}
};

this.currentModel = 'semantic-advanced';
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
if (!model || !model.analyze) return null;

return model.analyze(text);
}

embed(text) {
const model = this.models['semantic-advanced'];
if (!model || !model.embed) return Array(12).fill(0.5);

return model.embed(text);
}
}

// ============================================
// WARP v2.1 — ГЛАВНАЯ СИСТЕМА
// ============================================

class WarpSystem {
constructor() {
this.physics = new TensorPhysics(12);
this.localModel = new LocalModelEngine();
this.cycleCount = 0;
this.resonanceMemory = 0;
this.logs = [];
this.parameters = {
tau: 2.5,
kappa: 1.2,
n: 3
};

this.history = {
mass: [],
sigma: [],
force: [],
activity: [],
time: []
};
}

log(message, level = 'INFO') {
const timestamp = new Date().toLocaleTimeString('ru-RU', {
hour: '2-digit',
minute: '2-digit',
second: '2-digit'
});
const entry = `[${timestamp}] ${level}: ${message}`;
this.logs.push(entry);
if (this.logs.length > 150) this.logs.shift();
return entry;
}

async runCycle(inputText) {
this.cycleCount++;

// ℘: Восприятие
const perception = this.localModel.embed(inputText);
const analysis = this.localModel.analyze(inputText);

this.log(`Цикл #${this.cycleCount}: "${inputText.substring(0, 40)}..."`);
if (analysis) {
this.log(`Сложность: ${analysis.complexity.toFixed(3)}, Слов: ${analysis.wordCount}`);
}

// O: Образ
const oldImage = this.physics.history.length > 0
? this.physics.history[this.physics.history.length - 1][1]
: Array(12).fill(0.3);

// L → ε → R[n]: Физическое ядро
const result = this.physics.step(perception, oldImage);

// Обновление резонанса
this.resonanceMemory = result.sigma * 0.7 + this.resonanceMemory * 0.3;

// Сохранение истории
this.history.mass.push(result.mass);
this.history.sigma.push(result.sigma);
this.history.force.push(result.force);
this.history.activity.push(result.activity);
this.history.time.push(this.cycleCount);

const maxHistory = 60;
if (this.history.mass.length > maxHistory) {
this.history.mass.shift();
this.history.sigma.shift();
this.history.force.shift();
this.history.activity.shift();
this.history.time.shift();
}

return {
cycle: this.cycleCount,
mass: result.mass,
sigma: result.sigma,
force: result.force,
activity: result.activity,
resonance: this.resonanceMemory,
analysis: analysis,
perception: perception,
epsilon: result.epsilonList[0] || 0
};
}

getDynamisState(result) {
// Внутреннее напряжение
const tension = Math.min(result.force * result.sigma * 0.12, 6.0);

// Стабильность
const stability = 1.0 / (1.0 + result.mass * 0.15);

// Эмоциональный резонанс
const historyLength = this.history.force.length;
const previousForce = historyLength > 1 ? this.history.force[historyLength - 2] : 0;
const resonanceShift = result.force - previousForce;

let resonanceIndicator, color, symbol;

if (resonanceShift > 0.08) {
            resonanceIndicator = 'Активация ▲';
color = 'yellow';
symbol = '↑';
} else if (resonanceShift < -0.08) {
resonanceIndicator = 'Редукция ▼';
color = 'cyan';
symbol = '↓';
} else if (tension > 0.8) {
            resonanceIndicator = 'Напряжение ⚡';
color = 'red';
symbol = '⚡';
} else if (result.activity > 0.6) {
resonanceIndicator = 'Фокус ◎';
color = 'green';
symbol = '◎';
} else {
            resonanceIndicator = 'Спокойствие ○';
color = 'white';
symbol = '○';
}

// Когнитивное состояние
        let cognitiveState = 'Нейтральное';
if (result.mass > 1.5) cognitiveState = 'Сложное';
if (result.sigma > 2.0) cognitiveState = 'Глубокое';
if (tension > 1.5) cognitiveState = 'Напряженное';

return {
tension: tension.toFixed(4),
stability: Math.min(stability, 1.0).toFixed(4),
indicator: resonanceIndicator,
color: color,
symbol: symbol,
cognitive: cognitiveState,
shift: resonanceShift.toFixed(4)
};
}

getHistoryChartData() {
const recent = 30;
const start = Math.max(0, this.history.time.length - recent);

return {
time: this.history.time.slice(start),
mass: this.history.mass.slice(start),
sigma: this.history.sigma.slice(start),
force: this.history.force.slice(start)
};
}

reset() {
this.physics = new TensorPhysics(12);
this.cycleCount = 0;
this.resonanceMemory = 0;
this.history = { mass: [], sigma: [], force: [], activity: [], time: [] };
        this.log('Система сброшена в начальное состояние');
}

setParameter(param, value) {
if (this.parameters.hasOwnProperty(param)) {
const oldValue = this.parameters[param];
this.parameters[param] = parseFloat(value);
            this.log(`Параметр ${param} изменен: ${oldValue} → ${value}`);
return true;
}
return false;
}

getStatus() {
return {
cycle: this.cycleCount,
resonance: this.resonanceMemory,
parameters: {...this.parameters},
model: this.localModel.currentModel,
historySize: this.history.mass.length
};
}
}

// ============================================
// TUI С ASCII ГРАФИКАМИ
// ============================================

class WarpTUI {
constructor() {
this.system = new WarpSystem();
this.screen = blessed.screen({
smartCSR: true,
title: 'Warp v2.1 — Cognitive Terminal System',
fullUnicode: true,
cursor: {
artificial: true,
shape: 'line',
blink: true
}
});

this.initUI();
this.bindEvents();
this.updateDisplay();
}

initUI() {
// Основной контейнер
this.mainContainer = blessed.box({
parent: this.screen,
top: 0,
left: 0,
width: '100%',
height: '100%',
style: {
bg: 'black'
}
});

// Верхняя панель статуса
this.header = blessed.box({
parent: this.mainContainer,
top: 0,
left: 0,
width: '100%',
height: 3,
border: {
type: 'line'
},
style: {
border: {
fg: 'cyan'
},
bg: 'black'
}
});

this.headerText = blessed.text({
parent: this.header,
top: 0,
left: 2,
content: '',
style: {
fg: 'white',
bold: true
}
});

// Основная область с сеткой
this.grid = blessed.layout({
parent: this.mainContainer,
top: 3,
left: 0,
width: '100%',
height: '100%-6',
layout: 'grid',
border: {
type: 'bg'
}
});

// Панель состояния (левая верхняя)
this.stateBox = blessed.box({
parent: this.grid,
border: {
type: 'line'
},
style: {
border: {
fg: 'blue'
}
},
            label: ' {bold}Состояние{/bold} ',
tags: true
});

this.stateText = blessed.text({
parent: this.stateBox,
top: 0,
left: 1,
content: '',
tags: true
});

// Панель динамики (правая верхняя)
this.dynamicsBox = blessed.box({
parent: this.grid,
border: {
type: 'line'
},
style: {
border: {
fg: 'magenta'
}
},
            label: ' {bold}Динамика{/bold} ',
tags: true
});

this.dynamicsText = blessed.text({
parent: this.dynamicsBox,
top: 0,
left: 1,
content: '',
tags: true
});

// Лог (левая нижняя)
this.logBox = blessed.box({
parent: this.grid,
border: {
type: 'line'
},
style: {
border: {
fg: 'yellow'
}
},
            label: ' {bold}Лог системы{/bold} ',
tags: true
});

this.logList = blessed.list({
parent: this.logBox,
top: 0,
left: 1,
width: '100%-2',
height: '100%-2',
style: {
item: {
fg: 'gray'
},
selected: {
bg: 'blue',
fg: 'white'
}
},
scrollable: true,
keys: true,
mouse: true,
items: []
});

// ASCII графики (правая нижняя)
this.chartBox = blessed.box({
parent: this.grid,
border: {
type: 'line'
},
style: {
border: {
fg: 'green'
}
},
            label: ' {bold}Графики{/bold} ',
tags: true
});

this.chartText = blessed.text({
parent: this.chartBox,
top: 0,
left: 1,
content: '',
tags: true
});

// Поле ввода
this.inputBox = blessed.textbox({
parent: this.mainContainer,
bottom: 0,
left: 0,
width: '100%',
height: 3,
border: {
type: 'line'
},
style: {
border: {
fg: 'white'
},
bg: 'black',
fg: 'white'
},
label: ' {bold}Ввод (℘):{/bold} ',
inputOnFocus: true,
tags: true,
keys: true,
mouse: true
});

// Статус бар
this.statusBar = blessed.box({
parent: this.mainContainer,
bottom: 3,
left: 0,
width: '100%',
height: 1,
style: {
bg: 'blue',
fg: 'white'
}
});

this.statusText = blessed.text({
parent: this.statusBar,
content: '',
style: {
bold: true
}
});

// Настройка сетки
this.grid.grid.set(0, 0, 1, 1, this.stateBox);
this.grid.grid.set(0, 1, 1, 1, this.dynamicsBox);
this.grid.grid.set(1, 0, 1, 1, this.logBox);
this.grid.grid.set(1, 1, 1, 1, this.chartBox);
}

createAsciiChart(data, height = 8) {
        if (data.length === 0) return "Нет данных";

const max = Math.max(...data);
const min = Math.min(...data);
const range = max - min || 1;

const normalized = data.map(v =>
Math.round(((v - min) / range) * (height - 1))
);

const rows = [];
for (let y = height - 1; y >= 0; y--) {
let row = '';
for (let x = 0; x < normalized.length; x++) {
row += normalized[x] >= y ? '█' : ' ';
}
rows.push(row);
}

return rows.join('\n');
}

updateDisplay(result = null) {
// Заголовок
const status = this.system.getStatus();
this.headerText.setContent(
`WARP v2.1 | Цикл: ${status.cycle} | Модель: ${status.model} | ` +
`Резонанс: ${status.resonance.toExponential(3)}`
);

// Панель состояния
let stateContent = `{bold}Архитектура POLER[n]:{/bold}\n`;
        stateContent += `• ℘: Восприятие\n`;
stateContent += `• O: Образ\n`;
stateContent += `• L: Логика\n`;
