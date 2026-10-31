---
name: find-skills
description: >-
  Поиск и установка навыков (Agent Skills) из открытого каталога и репозиториев (Anthropic/Vercel skills ecosystem).
  Используй для обнаружения, скачивания и подключения новых специализированных навыков к агенту.
---

# 🔍 Find Skills — Agent Skills Discovery & Installation

Навык **find-skills** обучает агента находить, исследовать и устанавливать специализированные навыки (Agent Skills) из открытой экосистемы (Anthropic Skills standard, `skills.sh`, Vercel Labs, GitHub).

---

## 🚀 Основные команды и сценарии

### 1. Поиск навыков по ключевым словам:
```bash
npx skills find "<запрос>"
```
Или поиск по репозиториям GitHub / реестру `agentskills.io`.

### 2. Установка навыка в рабочее пространство:
```bash
npx skills add <user>/<repo>@<skill-name>
```
*Навыки автоматически сохраняются в каталог `.agents/skills/<name>/`.*

### 3. Ручной импорт из GitHub (Anthropic Spec):
Если навык находится в репозитории GitHub:
1. Клонировать или скачать директорию навыка с `SKILL.md`.
2. Поместить в `.agents/skills/<имя-навыка>/SKILL.md`.
3. Навык мгновенно становится доступным для агента через прогрессивное раскрытие (Progressive Disclosure).

---

## 📋 Формат совместимости
Все устанавливаемые навыки полностью соответствуют спецификации **Anthropic Agent Skills Spec** (`SKILL.md` с YAML frontmatter `name` + `description` и подробными инструкциями в Markdown).
