from flask import Flask, jsonify
import os

app = Flask(__name__, static_folder='.')

@app.route('/api/list')
def list_files():
return jsonify(os.listdir('.'))

app.run(port=3000)
