"""
POLER Knowledge Engine — SQLite FTS5 Search Database & EPUB Book Builder
Part of POLER[Ψ] Toolsuite.
"""

import os
import sys
import glob
import sqlite3
import subprocess
import argparse

def build_fts_database(specs_dir: str, db_output_path: str):
    """Builds a SQLite database with FTS5 full-text search index from technical specifications."""
    if os.path.exists(db_output_path):
        os.remove(db_output_path)

    conn = sqlite3.connect(db_output_path)
    cur = conn.cursor()

    cur.execute("""
    CREATE TABLE specs (
        id INTEGER PRIMARY KEY,
        title TEXT,
        category TEXT,
        invariant TEXT,
        filename TEXT,
        full_text TEXT
    )
    """)

    cur.execute("""
    CREATE VIRTUAL TABLE specs_fts USING fts5(
        id UNINDEXED,
        title,
        category,
        invariant,
        full_text,
        content='specs',
        content_rowid='id'
    )
    """)

    files = sorted(glob.glob(os.path.join(specs_dir, "PTS-*.md")))
    print(f"[*] Indexing {len(files)} specifications into SQLite FTS5 database...")

    for fpath in files:
        fname = os.path.basename(fpath)
        with open(fpath, "r", encoding="utf-8") as f:
            content = f.read()

        import re
        num_m = re.search(r"PTS-(\d+)", fname)
        num = int(num_m.group(1)) if num_m else 0

        title_m = re.search(r"^#\s+PTS-\d+:\s+ТЕХНИЧЕСКАЯ СПЕЦИФИКАЦИЯ\s+—\s+(.+)$", content, re.MULTILINE)
        title = title_m.group(1).strip() if title_m else fname

        cat_m = re.search(r">\s+\*\*Классификация:\*\*\s+`([^`]+)`", content)
        cat = cat_m.group(1).strip() if cat_m else "General"

        inv_m = re.search(r">\s+\*\*Базовый математический инвариант:\*\*\s+\$([^$]+)\$", content)
        inv = inv_m.group(1).strip() if inv_m else "H^\\Psi = 0"

        cur.execute("""
        INSERT INTO specs (id, title, category, invariant, filename, full_text)
        VALUES (?, ?, ?, ?, ?, ?)
        """, (num, title, cat, inv, fname, content))

    conn.commit()
    cur.execute("INSERT INTO specs_fts(id, title, category, invariant, full_text) SELECT id, title, category, invariant, full_text FROM specs")
    conn.commit()
    print(f"[+] SQLite FTS5 Database successfully created at: {db_output_path}")

def build_epub_book(specs_dir: str, output_epub_path: str, title="POLER Technical Specifications"):
    """Compiles markdown specifications into an EPUB book with interactive Table of Contents."""
    files = sorted(glob.glob(os.path.join(specs_dir, "PTS-*.md")))
    index_file = os.path.join(specs_dir, "PTS_MASTER_INDEX.md")
    
    all_inputs = []
    if os.path.exists(index_file):
        all_inputs.append(index_file)
    all_inputs.extend(files)

    print(f"[*] Compiling EPUB book '{title}' from {len(all_inputs)} documents...")
    cmd = [
        "pandoc",
        "--toc",
        "--toc-depth=2",
        "--metadata", f"title={title}",
        "--metadata", "author=POLER Core Research",
        "--metadata", "language=ru",
        "-o", output_epub_path
    ] + all_inputs

    res = subprocess.run(cmd, capture_output=True, text=True)
    if res.returncode == 0:
        sz = os.path.getsize(output_epub_path)
        print(f"[+] EPUB Book successfully generated: {output_epub_path} ({sz / 1024 / 1024:.2f} MB)")
    else:
        print(f"[-] Error compiling EPUB: {res.stderr[:500]}")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="POLER Database & EPUB Builder")
    parser.add_argument("--specs", required=True, help="Path to PTS specifications folder")
    parser.add_argument("--db", help="Path to output SQLite database")
    parser.add_argument("--epub", help="Path to output EPUB book")
    args = parser.parse_args()

    if args.db:
        build_fts_database(args.specs, args.db)
    if args.epub:
        build_epub_book(args.specs, args.epub)
