/** @type {import('next').NextConfig} */
const nextConfig = {
eslint: {
// Отключаем ESLint во время разработки для ускорения
ignoreDuringBuilds: true,
},
typescript: {
// Отключаем проверку типов (осторожно, может скрыть ошибки)
ignoreBuildErrors: true,
},
// Включаем экспериментальный турбопак (если используете Next.js 14+)
experimental: {
turbo: {},
},
};

module.exports = nextConfig;
