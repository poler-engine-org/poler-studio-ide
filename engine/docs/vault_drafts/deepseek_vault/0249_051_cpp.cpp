// В вашем движке, каждый кадр:
void GameEngine::OnFrameRender() {
// 1. Рендерим сцену в текстуру и ID-буфер
RenderSceneToTexture();
RenderIDBuffer();

// 2. Асинхронно читаем данные в RAM (без блокировки)
auto colorData = ReadbackTexture(renderTarget);
auto idData = ReadbackTexture(idBuffer);
auto depthData = ReadbackDepthBuffer();

// 3. Передаём в отдельный поток для CV-анализа
visionThread->Push(colorData, idData, depthData);
}

// В потоке CV:
void VisionProcessor::Process() {
auto [color, id, depth] = Pop();
// Сегментация по ID
    auto objects = SegmentByID(id, depth); // возвращает список с bbox, центром, глубиной
// Для каждого объекта вычисляем свойства
for (auto& obj : objects) {
obj.color = AverageColor(color, obj.bbox);
obj.orientation = ComputeOrientation(id, obj.bbox);
}
// Вычисляем отношения
auto relations = ComputeRelations(objects);
// Оптический поток
auto velocities = ComputeOpticalFlow(prevColor, color, objects);
// Формируем JSON
SceneDescription desc = BuildDescription(objects, relations, velocities);
// Отправляем LLM (через сокет или shared memory)
SendToLLM(desc);
}
