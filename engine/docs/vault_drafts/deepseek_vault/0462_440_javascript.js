async function runAnimation(scenes) {
const canvas = document.getElementById('mainCanvas');
const ctx = canvas.getContext('2d');
let currentTime = 0;
for (const scene of scenes) {
const startTime = performance.now();
// Отрисовка сцены
drawScene(ctx, scene.background, scene.characters);
// Запускаем озвучку
if (scene.dialogue) {
speak(scene.dialogue.text, { pitch: scene.dialogue.pitch, rate: scene.dialogue.rate });
}
// Ждём длительность сцены
await new Promise(r => setTimeout(r, scene.duration * 1000));
}
}
