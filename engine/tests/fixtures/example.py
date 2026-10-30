import json


class Worker:
    def run(self):
        data = self.load()
        return self.transform(data)

    def load(self):
        return [1, 2, 3]

    def transform(self, items):
        return [x * 2 for x in items]
