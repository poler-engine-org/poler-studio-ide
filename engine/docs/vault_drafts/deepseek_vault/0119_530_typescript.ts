const BlockRegistry: Map<string, ViewModelClass> = new Map();
BlockRegistry.set("term", TermViewModel);      // Терминал
BlockRegistry.set("waveai", WaveAiModel);      // AI панель
BlockRegistry.set("web", WebViewModel);        // Веб-просмотр
BlockRegistry.set("preview", PreviewModel);    // Превью
BlockRegistry.set("sysinfo", SysinfoViewModel); // Системная инфо
BlockRegistry.set("vdom", VDomModel);          // Виртуальный DOM
BlockRegistry.set("aifilediff", AiFileDiffViewModel); // AI diff
