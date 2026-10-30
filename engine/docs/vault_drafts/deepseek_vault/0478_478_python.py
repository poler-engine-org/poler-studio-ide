def save_checkpoint(self, filepath):
np.savez_compressed(filepath,
semantic_dm=self.semantic_dm,
L_matrix=self.L_matrix,
J_matrix=self.J_matrix,
resonance=self.resonance,
vocab_size=self.vocab_size,
eta0=self.eta0,
rho=self.rho,
)
# Сохраняем словарь и метаданные отдельно
meta = {
'known_words': list(self.known_words),
'chunks_processed': self.chunks_processed,
'articles_seen': self.articles_seen,
}
with open(filepath + '.meta.json', 'w') as f:
json.dump(meta, f)

def load_checkpoint(self, filepath):
data = np.load(filepath)
self.semantic_dm = data['semantic_dm']
self.L_matrix = data['L_matrix']
self.J_matrix = data['J_matrix']
self.resonance = data['resonance']
self.vocab_size = int(data['vocab_size'])
self.eta0 = float(data['eta0'])
self.rho = float(data['rho'])

with open(filepath + '.meta.json', 'r') as f:
meta = json.load(f)
self.known_words = set(meta['known_words'])
self.chunks_processed = meta['chunks_processed']
self.articles_seen = meta['articles_seen']
