# В конце orchestrator.py добавить аргумент --watch
if args.watch:
from watcher import ConversationWatcher
watcher = ConversationWatcher(skills_dir=Path(args.skills_dir))
watcher.start()
return

Таким образом, пользователь может запустить:

bash
