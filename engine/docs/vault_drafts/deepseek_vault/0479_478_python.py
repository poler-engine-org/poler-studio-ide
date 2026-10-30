from git import Repo

def push_to_github(self, repo_path, commit_msg="Update POLER checkpoint"):
repo = Repo(repo_path)
repo.index.add(['checkpoint.npz', 'checkpoint.npz.meta.json'])
repo.index.commit(commit_msg)
origin = repo.remote(name='origin')
origin.push()
