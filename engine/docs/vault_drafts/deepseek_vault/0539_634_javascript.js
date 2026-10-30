for (let i = 0; i < comet.tail.length; i++) {
const alpha = 1 - i / tailLength;
ctx.beginPath();
ctx.arc(comet.tail[i].x, comet.tail[i].y, comet.size * (1 - i * 0.03), 0, 2 * Math.PI);
ctx.fillStyle = `rgba(255, 255, 200, ${alpha*0.5})`;
ctx.fill();
}
