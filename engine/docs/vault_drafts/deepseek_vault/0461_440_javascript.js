function speak(text, options = {}) {
const utterance = new SpeechSynthesisUtterance(text);
utterance.lang = options.lang || 'ru-RU';
utterance.rate = options.rate || 1.0;  // скорость
utterance.pitch = options.pitch || 1.0; // высота
utterance.voice = speechSynthesis.getVoices()
.find(v => v.name.includes(options.voiceName || 'Google русский')) || null;

return new Promise(resolve => {
utterance.onend = resolve;
speechSynthesis.speak(utterance);
});
}
