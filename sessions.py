#!/usr/bin/env python3
"""Read the live state of local Claude Code sessions.

Sources (all written by Claude Code itself, read-only here):
  ~/.claude/sessions/<pid>.json      name, cwd, status (busy/idle/waiting), timestamps
  ~/.claude/projects/*/<id>.jsonl    transcript: tool calls, last prompt, last answer

Run directly to print the current state as JSON (add --watch to repeat, --demo for fake data).
"""
import glob
import json
import os
import re
import sys
import time

CLAUDE_DIR = os.path.expanduser(os.environ.get("CLAUDE_CONFIG_DIR", "~/.claude"))
TAIL_BYTES = 256 * 1024
HOME = os.path.expanduser("~")

# Claude Code status -> screensaver state
STATE_MAP = {
    "busy": "working",
    "working": "working",
    "running": "working",
    "waiting": "waiting",
    "requires_action": "waiting",
    "blocked": "blocked",
    "idle": "idle",
    "shell": "working",  # turn ended, but a background shell task is still running (e.g. a test suite)
}
BACKGROUND_STATUSES = {"shell"}

_transcript_cache = {}  # path -> ((size, mtime), info)


def _pid_alive(pid):
    try:
        pid = int(pid)
    except (TypeError, ValueError):
        return False
    if os.path.isdir("/proc/self"):
        return os.path.exists(f"/proc/{pid}")
    if sys.platform == "win32":
        import ctypes
        k32 = ctypes.windll.kernel32
        h = k32.OpenProcess(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
        if not h:
            return False
        code = ctypes.c_ulong()
        ok = k32.GetExitCodeProcess(h, ctypes.byref(code))
        k32.CloseHandle(h)
        return bool(ok) and code.value == 259  # STILL_ACTIVE
    try:  # macOS and other Unix
        os.kill(pid, 0)
        return True
    except PermissionError:
        return True
    except OSError:
        return False


def _short_path(path):
    if path and (path == HOME or path.startswith(HOME + os.sep)):
        return "~" + path[len(HOME):]
    return path or ""


def _find_transcript(session_id, cwd):
    if not session_id:
        return None
    if cwd:
        p = os.path.join(CLAUDE_DIR, "projects", re.sub(r"[^A-Za-z0-9]", "-", cwd), session_id + ".jsonl")
        if os.path.exists(p):
            return p
    hits = glob.glob(os.path.join(CLAUDE_DIR, "projects", "*", session_id + ".jsonl"))
    return hits[0] if hits else None


def _one_line(text, limit=120):
    return " ".join(str(text).split())[:limit]


def _tool_detail(inp):
    if not isinstance(inp, dict):
        return ""
    for key in ("description", "command", "file_path", "path", "pattern", "url", "query", "skill", "prompt"):
        v = inp.get(key)
        if isinstance(v, str) and v.strip():
            if key in ("file_path", "path"):
                v = os.path.basename(v.rstrip("/")) or v
            return _one_line(v, 80)
    return ""


def _parse_transcript(path):
    info = {"tool": None, "tool_detail": "", "last_text": "", "prompt": "", "tool_events": [], "bg_task": ""}
    try:
        st = os.stat(path)
    except OSError:
        return info
    key = (st.st_size, st.st_mtime)
    cached = _transcript_cache.get(path)
    if cached and cached[0] == key:
        return cached[1]
    try:
        with open(path, "rb") as f:
            offset = max(0, st.st_size - TAIL_BYTES)
            f.seek(offset)
            data = f.read().decode("utf-8", "replace")
    except OSError:
        return info
    lines = data.splitlines()
    if offset > 0 and lines:
        lines = lines[1:]  # first line is probably cut in half
    events = []
    for line in lines:
        try:
            obj = json.loads(line)
        except ValueError:
            continue
        typ = obj.get("type")
        msg = obj.get("message") or {}
        content = msg.get("content")
        if typ == "assistant" and isinstance(content, list):
            for item in content:
                if not isinstance(item, dict):
                    continue
                if item.get("type") == "tool_use":
                    name = item.get("name") or "tool"
                    detail = _tool_detail(item.get("input"))
                    events.append((item.get("id") or f"{len(events)}", name, detail))
                    info["tool"], info["tool_detail"] = name, detail
                    if isinstance(item.get("input"), dict) and item["input"].get("run_in_background"):
                        info["bg_task"] = detail or name  # the latest task started in the background
                elif item.get("type") == "text" and item.get("text", "").strip():
                    info["last_text"] = _one_line(item["text"])
        elif typ == "user" and not obj.get("isMeta") and not obj.get("isSidechain"):
            text = None
            if isinstance(content, str):
                text = content
            elif isinstance(content, list):
                parts = [c.get("text", "") for c in content if isinstance(c, dict) and c.get("type") == "text"]
                if parts and not any(isinstance(c, dict) and c.get("type") == "tool_result" for c in content):
                    text = " ".join(parts)
            if text and text.strip() and not text.lstrip().startswith("<"):
                info["prompt"] = _one_line(text)
    info["tool_events"] = events[-30:]
    _transcript_cache[path] = (key, info)
    return info


def read_sessions():
    """Return a list of dicts describing every live Claude Code session."""
    result = []
    seen_paths = set()
    for path in sorted(glob.glob(os.path.join(CLAUDE_DIR, "sessions", "*.json"))):
        try:
            with open(path, encoding="utf-8") as f:
                raw = json.load(f)
        except (OSError, ValueError):
            continue
        if not _pid_alive(raw.get("pid")):
            continue  # stale file of a session that has ended
        cwd = raw.get("cwd") or ""
        sid = raw.get("sessionId") or str(raw.get("pid"))
        status = raw.get("status") or "idle"
        since_ms = raw.get("statusUpdatedAt") or raw.get("updatedAt") or raw.get("startedAt")
        tpath = _find_transcript(sid, cwd)
        if tpath:
            seen_paths.add(tpath)
        t = _parse_transcript(tpath) if tpath else {}
        waiting_for = raw.get("waitingFor")
        if isinstance(waiting_for, dict):
            waiting_for = waiting_for.get("type") or waiting_for.get("kind") or "input"
        result.append({
            "id": sid,
            "pid": raw.get("pid"),
            "name": raw.get("name") or os.path.basename(cwd.rstrip("/")) or f"session {raw.get('pid')}",
            "cwd": _short_path(cwd),
            "status": status,
            "state": STATE_MAP.get(status, "idle"),
            "background": status in BACKGROUND_STATUSES,
            "bg_task": t.get("bg_task", ""),
            "waiting_for": str(waiting_for) if waiting_for else "",
            "since": (since_ms / 1000.0) if since_ms else time.time(),
            "started": (raw.get("startedAt") or 0) / 1000.0,
            "tool": t.get("tool"),
            "tool_detail": t.get("tool_detail", ""),
            "last_text": t.get("last_text", ""),
            "prompt": t.get("prompt", ""),
            "tool_events": t.get("tool_events", []),
        })
    for p in list(_transcript_cache):
        if p not in seen_paths:
            del _transcript_cache[p]
    return result


# --------------------------------------------------------------------------- demo data

_DEMO_START = time.time()
_DEMO = [
    ("Netdocs Test 3", "~/dev/netdocs", 0),
    ("Screensaver", "~/dev/claude-screensaver", 13),
    ("Infra audit", "~/dev/infra", 27),
]
_DEMO_TOOLS = [("Read", "inventory.yaml"), ("Grep", "def parse_"), ("Bash", "Run unit tests"),
               ("Edit", "scene.py"), ("Glob", "**/*.md"), ("Bash", "git status"), ("Write", "README.md"),
               ("WebFetch", "docs.python.org"), ("Agent", "Explore network configs")]
_DEMO_TEXT = ["All 42 tests pass. Want me to commit?", "Done - the parser now handles empty lines.",
              "I found three hosts with outdated firmware."]


def demo_sessions(t=None):
    """Fake sessions cycling through all states, for previews."""
    now = time.time()
    if t is None:
        t = now - _DEMO_START
    out = []
    period = 42.0
    for i, (name, cwd, off) in enumerate(_DEMO):
        p = (t + off) % period
        if p < 20:
            state, start = "working", 0
        elif p < 29:
            state, start = "waiting", 20
        else:
            state, start = "idle", 29
        # tool calls only happen while working; afterwards the list is frozen
        n = int((t + off - max(0.0, p - 20)) / 2.3)
        events = [(f"{i}-{k}", *_DEMO_TOOLS[(k * 7 + i) % len(_DEMO_TOOLS)]) for k in range(max(0, n - 6), n + 1)]
        tool, detail = events[-1][1], events[-1][2]
        out.append({
            "id": f"demo-{i}", "pid": 1000 + i, "name": name, "cwd": cwd,
            "status": {"working": "busy"}.get(state, state), "state": state,
            "waiting_for": "permission to run Bash" if state == "waiting" else "",
            "since": now - (p - start), "started": now - 3600,
            "tool": tool, "tool_detail": detail,
            "last_text": _DEMO_TEXT[i % len(_DEMO_TEXT)],
            "prompt": "Refactor the parser and add tests",
            "tool_events": events,
            "background": False, "bg_task": "",
        })
    out.append({  # a test bench: turn finished, suite still running as a background task
        "id": "demo-bg", "pid": 1099, "name": "Test bench", "cwd": "~/dev/netdocs",
        "status": "shell", "state": "working", "background": True,
        "bg_task": "Wait for all test rows to finish", "waiting_for": "",
        "since": now - (t % 900) - 120, "started": now - 7200,
        "tool": "Bash", "tool_detail": "Wait for all test rows to finish",
        "last_text": "The test rows are running in the background.", "prompt": "Run the test bench",
        "tool_events": [("bg-0", "Bash", "Wait for all test rows to finish")],
    })
    return out


if __name__ == "__main__":
    demo = "--demo" in sys.argv
    while True:
        data = demo_sessions() if demo else read_sessions()
        for s in data:
            s["tool_events"] = len(s["tool_events"])
        print(json.dumps(data, indent=2, ensure_ascii=False))
        if "--watch" not in sys.argv:
            break
        time.sleep(1)
        print("\033[2J\033[H", end="")
