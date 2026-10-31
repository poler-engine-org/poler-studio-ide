"""
POLER Knowledge Engine — High-Speed CUDA Audio/Video Ingestion Engine
Part of POLER[Ψ] Toolsuite.

Automated batch downloader & GPU int8 Whisper transcriber.
Runs directly on NVIDIA CUDA (Pascal/GTX 1060+) with dynamic cuBLAS/cuDNN loader.
"""

import os
import sys
import json
import time
import subprocess
import glob
import argparse

def init_cuda_runtime():
    """Dynamically loads installed NVIDIA CUDA/cuBLAS/cuDNN runtime libraries."""
    site_packages = [p for p in sys.path if 'site-packages' in p]
    for sp in site_packages:
        for pkg in ['nvidia/cublas/lib', 'nvidia/cudnn/lib', 'nvidia/cuda_nvrtc/lib']:
            full_p = os.path.join(sp, pkg)
            if os.path.exists(full_p):
                os.environ['LD_LIBRARY_PATH'] = full_p + ':' + os.environ.get('LD_LIBRARY_PATH', '')

    import ctypes
    for sp in site_packages:
        for pkg in ['nvidia/cublas/lib', 'nvidia/cudnn/lib']:
            p = os.path.join(sp, pkg)
            if os.path.exists(p):
                for f in os.listdir(p):
                    if f.endswith('.so') or '.so.' in f:
                        try:
                            ctypes.CDLL(os.path.join(p, f))
                        except:
                            pass

def process_channel_or_urls(urls_file: str, out_dir: str, model_size="base"):
    init_cuda_runtime()
    from faster_whisper import WhisperModel

    audio_dir = os.path.join(out_dir, "audio")
    transcripts_dir = os.path.join(out_dir, "transcripts")
    os.makedirs(audio_dir, exist_ok=True)
    os.makedirs(transcripts_dir, exist_ok=True)

    print(f"[*] Initializing Whisper ({model_size}) on GPU (CUDA int8)...")
    model = WhisperModel(model_size, device='cuda', compute_type='int8')
    print("[+] Model loaded into GPU memory.")

    # Read items
    items = []
    with open(urls_file, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line: continue
            if line.startswith("{"):
                d = json.loads(line)
                items.append((d.get("id"), d.get("title", ""), d.get("url", f"https://www.youtube.com/shorts/{d.get('id')}")))
            else:
                items.append((line.split("/")[-1], line, line))

    print(f"[*] Processing {len(items)} items...")
    for idx, (vid, title, url) in enumerate(items, 1):
        md_file = os.path.join(transcripts_dir, f"{vid}.md")
        if os.path.exists(md_file):
            continue

        print(f"[{idx}/{len(items)}] Downloading & Transcribing {vid}: {title[:40]}...")
        audio_file = os.path.join(audio_dir, f"{vid}.mp3")
        
        if not os.path.exists(audio_file):
            try:
                cmd = [
                    sys.executable, "-m", "yt_dlp",
                    "--js-runtimes", "node:/usr/bin/node",
                    "-f", "ba/b",
                    "--extract-audio", "--audio-format", "mp3",
                    url,
                    "-o", os.path.join(audio_dir, f"{vid}.%(ext)s")
                ]
                subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=45)
            except Exception as e:
                print(f"  [Download Error]: {e}")

        # Check existing audio
        if not os.path.exists(audio_file):
            possible = glob.glob(os.path.join(audio_dir, f"{vid}.*"))
            if possible: audio_file = possible[0]

        if os.path.exists(audio_file):
            try:
                t0 = time.time()
                segments, info = model.transcribe(audio_file, language='ru')
                snip_dicts = []
                texts = []
                for s in segments:
                    snip_dicts.append({"text": s.text.strip(), "start": round(s.start, 2), "end": round(s.end, 2)})
                    texts.append(s.text.strip())
                
                full_text = " ".join(texts)
                print(f"  [+] Transcribed in {time.time()-t0:.1f}s ({len(full_text)} chars)")
                
                with open(md_file, "w", encoding="utf-8") as tf:
                    tf.write(f"# {title}\n\n")
                    tf.write(f"- **URL:** [{url}]({url})\n")
                    tf.write(f"- **ID:** `{vid}`\n\n")
                    tf.write("## 📝 Полный текст:\n\n")
                    tf.write(full_text + "\n\n")
                    tf.write("## ⏱️ Таймкоды:\n\n")
                    for snip in snip_dicts:
                        m = int(snip['start'] // 60)
                        s = int(snip['start'] % 60)
                        tf.write(f"- `[{m:02d}:{s:02d}]` {snip['text']}\n")
            except Exception as e:
                print(f"  [Transcribe Error]: {e}")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="POLER GPU Audio/Video Transcriber")
    parser.add_argument("--urls", required=True, help="JSONL or TXT list of URLs/items")
    parser.add_argument("--out", required=True, help="Destination directory")
    parser.add_argument("--model", default="base", help="Whisper model size (tiny, base, small)")
    args = parser.parse_args()
    process_channel_or_urls(args.urls, args.out, args.model)
