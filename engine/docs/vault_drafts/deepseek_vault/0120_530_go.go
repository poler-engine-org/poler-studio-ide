type AIBackend interface {
StreamCompletion(ctx context.Context,
request wshrpc.WaveAIStreamRequest)
chan wshrpc.RespOrErrorUnion[wshrpc.WaveAIPacketType]
}

// Поддерживаемые провайдеры:
const APIType_Anthropic = "anthropic"
const APIType_Perplexity = "perplexity"
const APIType_Google = "google"
const APIType_OpenAI = "openai"
// + Local AI через OpenAI-compatible API (Ollama, LocalAI)
