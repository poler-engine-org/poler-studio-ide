// =============================================================================
// P³ ENGINE — C-ABI BRIDGE HEADER FOR O3DE LAUNCHER SOURCE COMPILATION
// =============================================================================
#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
#include <stdbool.h>

// 1. Версия и системная инициализация
const char* P3_GetBridgeVersion(void);
void O3DELauncher_IncreaseResourceLimits(void);
uint8_t O3DELauncher_RunNative(const char* engine_path, const char* project_path);

// 2. Спавн уровней и сущностей (Spawnable / ECS)
void* P3_Spawnable_Create(void);
void P3_Spawnable_Destroy(void* ptr);
bool P3_Spawnable_LoadFromFile(void* ptr, const char* path);
uint32_t P3_Spawnable_GetEntityCount(const void* ptr);

// 3. Камера и проекционный вьюпорт
void* P3_Camera_Create(void);
void P3_Camera_Destroy(void* ptr);
void P3_Camera_SetPivot(void* ptr, float x, float y, float z);
void P3_Camera_SetRotation(void* ptr, float yaw, float pitch);
void P3_Camera_Orbit(void* ptr, float delta_yaw, float delta_pitch);

// 4. Стек системного и UI курсора
void* P3_Cursor_Create(void);
void P3_Cursor_Destroy(void* ptr);
void P3_Cursor_SetSystemCursorState(void* ptr, uint8_t state);
uint8_t P3_Cursor_GetSystemCursorState(const void* ptr);
void P3_Cursor_IncrementVisibleCounter(void* ptr);
void P3_Cursor_DecrementVisibleCounter(void* ptr);
bool P3_Cursor_IsUiCursorVisible(const void* ptr);

// 5. Трансформации (PGL4 на S³) и Сущности
void* P3_Transform_Create(uint64_t entity_id);
void P3_Transform_Destroy(void* ptr);
void P3_Transform_SetLocalTranslation(void* ptr, float x, float y, float z);
void P3_Transform_SetLocalRotation(void* ptr, float qx, float qy, float qz, float qw);

void* P3_Entity_Create(uint64_t id, const char* name);
void P3_Entity_Destroy(void* ptr);
void P3_Entity_Init(void* ptr);
void P3_Entity_Activate(void* ptr);
void P3_Entity_Deactivate(void* ptr);

// 6. 3D Меши
void* P3_Mesh_Create(uint64_t entity_id, const char* path);
void P3_Mesh_Destroy(void* ptr);
uint32_t P3_Mesh_GetVertexCount(const void* ptr);
uint32_t P3_Mesh_GetIndexCount(const void* ptr);

#ifdef __cplusplus
}
#endif
