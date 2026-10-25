# PandaCode's bridge to Modal sandboxes (src-tauri/src/cloud/modal.rs). Reads one JSON
# request on stdin, prints one JSON reply on stdout. Credentials arrive only as
# MODAL_TOKEN_ID / MODAL_TOKEN_SECRET (and MODAL_ENVIRONMENT) in this process's environment.
import sys

# The caller also uses Python's isolated mode. Keep the current directory off the
# module path so a workspace json.py or modal.py never loads.
sys.path = [p for p in sys.path if p not in ("", ".")]

import json
import os
import time

TAG = {"pandacode": "sandbox"}
LIFETIME = 24 * 60 * 60
# PID 1 of every sandbox: runs until the deadline `keep_alive` writes, then exits, which
# ends the sandbox. No deadline file means run until Modal's own timeout.
WATCH = 'while [ "$(date +%s)" -lt "$(cat /tmp/.pandacode-deadline 2>/dev/null || echo 99999999999)" ]; do sleep 20; done'


def app(modal, create=False):
    environment = os.environ.get("MODAL_ENVIRONMENT") or None
    return modal.App.lookup("pandacode", create_if_missing=create, environment_name=environment)


def run(request):
    try:
        import modal
    except ImportError:
        return {"error": "missing-modal"}
    op = request["op"]
    if op == "check":
        # Authenticate without creating an App or a Sandbox.
        next(iter(modal.Sandbox.list(tags=TAG)), None)
        return {"checked": True}
    if op == "list":
        try:
            existing = app(modal)
        except modal.exception.NotFoundError:
            return {"sandboxes": []}
        found = modal.Sandbox.list(app_id=existing.app_id, tags=TAG)
        return {"sandboxes": [{"id": sb.object_id, "name": sb.get_tags().get("name", "")} for sb in found]}
    if op == "create":
        # Use a prepared image. Creating a machine never installs guest packages.
        image = modal.Image.from_registry(request["image"])
        sb = modal.Sandbox.create("sh", "-c", WATCH, app=app(modal, create=True), image=image, timeout=LIFETIME, cpu=2.0, memory=4096)
        # Modal ends the sandbox at its timeout; remember when, so PandaCode knows how long a run may last.
        sb.set_tags(dict(TAG, name=request["name"], expires=str(int(time.time()) + LIFETIME)))
        return {"id": sb.object_id}
    sb = modal.Sandbox.from_id(request["id"])
    if op == "running":
        expires = sb.get_tags().get("expires", "")
        remaining = int(expires) - int(time.time()) if expires.isdigit() else None
        return {"running": sb.poll() is None, "remaining": remaining}
    if op == "terminate":
        sb.terminate()
        return {}
    if op == "exec":
        process = sb.exec("sh", "-c", "exec 2>&1\n" + request["command"], timeout=int(request["timeout"]))
        output = b""
        for chunk in process.stdout:
            output = (output + chunk.encode("utf-8"))[-4 * 1024 * 1024:]
        process.wait()
        return {"code": process.returncode, "output": output.decode("utf-8", errors="replace")}
    return {"error": "unknown request " + op}


try:
    reply = run(json.load(sys.stdin))
except Exception as error:  # every failure becomes a readable reply
    reply = {"error": "%s: %s" % (type(error).__name__, error)}
print(json.dumps(reply))
