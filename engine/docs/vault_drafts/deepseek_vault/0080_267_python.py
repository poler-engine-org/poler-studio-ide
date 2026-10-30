# skill_server.py
from flask import Flask, request, jsonify
app = Flask(__name__)

@app.route('/run/<skill_name>', methods=['POST'])
def run_skill(skill_name):
data = request.json
result = orchestrator.run_skill(skill_name, data)
return jsonify(result)
