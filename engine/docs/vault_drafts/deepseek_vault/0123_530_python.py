class RepoMap:
def __init__(self, map_tokens=1024, ...):
self.max_map_tokens = map_tokens
# Использует PageRank для ранжирования!

def get_ranked_tags(self, ...):
# Создаёт граф зависимостей
G = nx.MultiDiGraph()
# Добавляет рёбра: referencer -> definer
G.add_edge(referencer, definer, weight=use_mul * num_refs)
# PageRank для определения важности
ranked = nx.pagerank(G, weight="weight", **pers_args)
