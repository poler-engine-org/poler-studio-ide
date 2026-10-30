function drawCharacter(ctx, x, y, faceType) {
// Голова
ctx.fillStyle = "#ffe0bd";
ctx.beginPath(); ctx.arc(x, y, 40, 0, Math.PI*2); ctx.fill();
// Глаза
const eyeY = y - 10;
ctx.fillStyle = "#000";
if (faceType === "happy") {
ctx.beginPath(); ctx.arc(x-15, eyeY, 5, 0, Math.PI, false); ctx.fill(); // ^_^
ctx.beginPath(); ctx.arc(x+15, eyeY, 5, 0, Math.PI, false); ctx.fill();
} else if (faceType === "angry") {
// ... angled eyebrows & narrow eyes
}
// ... волосы, рот
}
