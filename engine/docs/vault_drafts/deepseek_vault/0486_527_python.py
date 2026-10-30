from notebooklm import NotebookLM

client = NotebookLM(cookies_file="cookies.txt")
notebooks = client.list_notebooks()
