async loadExternalModel(modelPath) {
// Подключение через require
const externalModel = require(modelPath);
this.models['external'] = externalModel;
}

Для сохранения состояния:

javascript
