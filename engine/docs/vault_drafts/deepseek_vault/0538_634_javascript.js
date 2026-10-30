for (let i = 0; i < 200; i++) {
    if (i % 2 === 0) continue; // не тратить время
ctx.beginPath();
ctx.arc(Math.random() * width, Math.random() * height, Math.random() * 1.5, 0, 2 * Math.PI);
ctx.fill();
}
