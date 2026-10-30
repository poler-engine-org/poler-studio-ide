let mediaRecorder;
let recordedChunks = [];

async function startRecording(canvas) {
const stream = canvas.captureStream(30); // 30 FPS
// Микшируем со звуком из Web Audio (AudioContext)
const audioCtx = new AudioContext();
const dest = audioCtx.createMediaStreamDestination();
// ... подключаем все звуки к dest
const combinedStream = new MediaStream([
...stream.getVideoTracks(),
...dest.stream.getAudioTracks()
]);

mediaRecorder = new MediaRecorder(combinedStream, { mimeType: 'video/webm' });
mediaRecorder.ondataavailable = (e) => recordedChunks.push(e.data);
mediaRecorder.onstop = () => {
const blob = new Blob(recordedChunks, { type: 'video/webm' });
const url = URL.createObjectURL(blob);
// Создаём ссылку для скачивания
const a = document.createElement('a'); a.href = url; a.download = 'anime.webm'; a.click();
};
mediaRecorder.start();
}

// В конце анимации вызываем mediaRecorder.stop()
