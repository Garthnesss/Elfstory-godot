#!/usr/bin/env python3
"""Machine-wide queue for local AI asset generation (ComfyUI, TRELLIS.2).

One generation job runs at a time on this Mac, whichever AI session or
chat started it. Every job takes a ticket in ~/.asset-gen-queue/ and runs
only when (1) its ticket is first in line, (2) it holds the slot lock,
and (3) the services are actually idle — the foundry has no running job
and ComfyUI's prompt queue is empty — so a session that skipped the
queue still blocks everyone else instead of being trampled.

  genq.py status
  genq.py run --label "concept fig_rat" -- python3 tools/comfy_runner.py --mode concept ...
  genq.py foundry --image /abs/cutout.png --name fig_rat [--pipeline 512] [--tex 1024]

`foundry` submits to Elf Works /foundry (so the dashboard shows it) and
holds the slot until the job is done, failed or stopped. Exit code is 0
only when the wrapped job succeeded. Tickets of dead processes are
pruned automatically; Ctrl-C leaves the queue cleanly.
"""
import argparse
import fcntl
import json
import os
import re
import signal
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

QDIR = Path(os.environ.get("ASSET_GEN_QUEUE", str(Path.home() / ".asset-gen-queue")))
TICKETS = QDIR / "tickets"
LOCK = QDIR / "slot.lock"
CURRENT = QDIR / "current.json"
FOUNDRY = os.environ.get("FOUNDRY_URL", "http://127.0.0.1:8765")
COMFY = os.environ.get("COMFYUI_URL", "http://127.0.0.1:8188")
POLL = 3.0


def _get_json(url, timeout=4):
    try:
        with urllib.request.urlopen(url, timeout=timeout) as r:
            return json.loads(r.read().decode("utf-8"))
    except Exception:
        return None


def foundry_key():
    try:
        with urllib.request.urlopen(FOUNDRY + "/foundry", timeout=5) as r:
            html = r.read().decode("utf-8", "replace")
        m = re.search(r"OPS_KEY = '([^']*)'", html)
        return m.group(1) if m else ""
    except Exception:
        return ""


def foundry_state(key=None):
    key = key if key is not None else foundry_key()
    d = _get_json(f"{FOUNDRY}/api/foundry?k={key}") if key else None
    if not d or "job" not in d:
        return None
    return d["job"]


def services_busy():
    """Why the services aren't idle right now, or "" when they are."""
    job = foundry_state()
    if job and job.get("state") == "running":
        return f"foundry is sculpting '{job.get('name')}'"
    q = _get_json(COMFY + "/queue", timeout=2)
    if q and (q.get("queue_running") or q.get("queue_pending")):
        return f"ComfyUI has {len(q.get('queue_running', []))} running / {len(q.get('queue_pending', []))} pending prompts"
    return ""


def _alive(pid):
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False
    except PermissionError:
        return True


def tickets():
    TICKETS.mkdir(parents=True, exist_ok=True)
    out = []
    for f in sorted(TICKETS.glob("*.json")):
        try:
            t = json.loads(f.read_text())
        except Exception:
            continue
        if not _alive(int(t.get("pid", 0))):
            f.unlink(missing_ok=True)  # dead session: drop its place in line
            continue
        t["_file"] = f
        out.append(t)
    return out


class Slot:
    """Ticket + lock; context manager that waits for its turn."""

    def __init__(self, label, cmd):
        TICKETS.mkdir(parents=True, exist_ok=True)
        self.label = label
        self.file = TICKETS / f"{time.time_ns()}-{os.getpid()}.json"
        self.file.write_text(json.dumps({
            "pid": os.getpid(), "label": label, "cmd": cmd, "cwd": os.getcwd(),
            "queued_at": time.strftime("%Y-%m-%d %H:%M:%S"),
        }))
        self.lock_fd = None

    def __enter__(self):
        last = None
        while True:
            line = tickets()
            ahead = [t for t in line if t["_file"].name < self.file.name]
            why = ""
            if ahead:
                why = f"{len(ahead)} job(s) ahead, next: {ahead[0]['label']}"
            else:
                fd = os.open(LOCK, os.O_CREAT | os.O_RDWR)
                try:
                    fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
                except BlockingIOError:
                    os.close(fd)
                    why = "slot held by " + _current_label()
                else:
                    busy = services_busy()
                    if busy:
                        fcntl.flock(fd, fcntl.LOCK_UN)
                        os.close(fd)
                        why = busy + " (started outside the queue)"
                    else:
                        self.lock_fd = fd
                        CURRENT.write_text(json.dumps({
                            "pid": os.getpid(), "label": self.label,
                            "started_at": time.strftime("%Y-%m-%d %H:%M:%S"),
                        }))
                        print(f"GENQ running: {self.label}", flush=True)
                        return self
            if why != last:
                print(f"GENQ waiting: {why}", flush=True)
                last = why
            time.sleep(POLL)

    def __exit__(self, *exc):
        if self.lock_fd is not None:
            CURRENT.unlink(missing_ok=True)
            fcntl.flock(self.lock_fd, fcntl.LOCK_UN)
            os.close(self.lock_fd)
        self.file.unlink(missing_ok=True)
        return False


def _current_label():
    try:
        return json.loads(CURRENT.read_text()).get("label", "?")
    except Exception:
        return "another job"


def cmd_status(_a):
    try:
        cur = json.loads(CURRENT.read_text())
        if not _alive(int(cur.get("pid", 0))):
            cur = None
    except Exception:
        cur = None
    print("running:", f"{cur['label']} (pid {cur['pid']}, since {cur['started_at']})" if cur else "nothing")
    line = [t for t in tickets() if not cur or t["pid"] != cur["pid"]]
    print(f"queued: {len(line)}")
    for t in line:
        print(f"  - {t['label']} (pid {t['pid']}, queued {t['queued_at']})")
    busy = services_busy()
    print("services:", busy or "idle")


def cmd_run(a):
    if not a.cmd:
        sys.exit("genq run: give the command after --")
    with Slot(a.label or " ".join(a.cmd)[:80], a.cmd):
        # GENQ_SLOT tells self-queuing tools (comfy_runner.py) they already
        # hold the slot and must not queue again.
        rc = subprocess.call(a.cmd, env={**os.environ, "GENQ_SLOT": a.label or "1"})
    print(f"GENQ done: exit {rc}", flush=True)
    sys.exit(rc)


def cmd_foundry(a):
    img = str(Path(a.image).expanduser().resolve())
    if not Path(img).is_file():
        sys.exit(f"genq foundry: no such image {img}")
    with Slot(a.label or f"foundry {a.name}", ["foundry", img, a.name]):
        key = foundry_key()
        if not key:
            sys.exit("genq foundry: Elf Works /foundry is not reachable")
        body = json.dumps({"op": "start", "image_path": img, "output_name": a.name,
                           "pipeline_type": a.pipeline, "texture_size": a.tex}).encode()
        req = urllib.request.Request(f"{FOUNDRY}/api/foundry?k={key}", data=body,
                                     headers={"Content-Type": "application/json"}, method="POST")
        try:
            with urllib.request.urlopen(req, timeout=30) as r:
                print("GENQ submitted:", r.read().decode()[:200], flush=True)
        except urllib.error.HTTPError as e:
            sys.exit(f"genq foundry: start refused ({e.code}): {e.read().decode()[:300]}")
        time.sleep(5)
        last_log = ""
        while True:
            job = foundry_state(key) or {}
            st = job.get("state")
            log = (job.get("log") or [""])[-1]
            if log != last_log and a.verbose:
                print("  ", log[:140], flush=True)
                last_log = log
            if st in ("done", "failed", "stopped"):
                print(f"GENQ foundry {st}: {job.get('output')} elapsed {job.get('elapsed')}s "
                      f"error={job.get('error')}", flush=True)
                sys.exit(0 if st == "done" else 1)
            time.sleep(10)


def main():
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="op", required=True)
    sub.add_parser("status").set_defaults(fn=cmd_status)
    r = sub.add_parser("run")
    r.add_argument("--label", default="")
    r.add_argument("cmd", nargs=argparse.REMAINDER)
    r.set_defaults(fn=cmd_run)
    f = sub.add_parser("foundry")
    f.add_argument("--image", required=True)
    f.add_argument("--name", required=True)
    f.add_argument("--pipeline", default="512")
    f.add_argument("--tex", type=int, default=1024)
    f.add_argument("--label", default="")
    f.add_argument("--verbose", action="store_true")
    f.set_defaults(fn=cmd_foundry)
    a = p.parse_args()
    if getattr(a, "cmd", None) and a.cmd[0] == "--":
        a.cmd = a.cmd[1:]
    a.fn(a)


if __name__ == "__main__":
    main()
