import subprocess
import os
import sys
import tempfile

# Define the code string (I will just write it into the file)
code = """...""" # Actually, to avoid token overflow in the tool call, I'll do a simpler approach: use subprocess to call python -c with the code? No, that's huge.
# Better: Create /tmp/poler5.py and write the code to it.
# Wait, the code is HUGE. I will write it to a file directly using open().write().

# The code is provided in the user's prompt. I will extract it in the python tool.
# Actually, to save tokens, I'll just use the `python` tool to create the file.
# But the python tool execution might have a time limit, but fine.

Let's structure the test:

Write the user's script to /tmp/poler5.py.

Write a test file /tmp/sample.txt with some lines.

Import the script's functions and test them.
Wait, I must be careful with #!/usr/bin python3 shebang. It's not needed.

Let's execute the code piece by piece to ensure it works. I will use the python tool.

Wait! The user provided the entire code in the prompt. I will paste it inside the python tool execution block and write it to a file.

Let's proceed.
