#!/usr/bin/env python3
"""Check Relay's native responsive layout on X11 using only its own window.

Uses a fresh private data directory; never opens a provider or signs in. Requires
wmctrl, Python GI/AT-SPI, and an active desktop. For example:
  python3 tests/responsive_smoke.py --output-dir /tmp/relay-responsive-qa
"""

import argparse
import itertools
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time

from native_smoke import (capture_window, inspect_accessibility, owned_windows,
                          process_tree, renderer, same_process)

SERVICES = ["WhatsApp", "Telegram", "Instagram", "LinkedIn", "Gmail"]
CONTROLS = SERVICES + ["Sleep apps", "Quit"]
SIZES = [("wide", 1100, 740), ("medium", 640, 480), ("narrow", 420, 640),
         ("small", 320, 240), ("short", 1100, 320), ("restored", 1100, 740)]


def edges(box):
    return box["x"], box["y"], box["x"] + box["width"], box["y"] + box["height"]


def contains(outer, inner, tolerance=2):
    a, b, c, d = edges(outer)
    x, y, right, bottom = edges(inner)
    return x >= a - tolerance and y >= b - tolerance and right <= c + tolerance and bottom <= d + tolerance


def overlaps(first, second):
    a, b, c, d = edges(first)
    x, y, right, bottom = edges(second)
    return min(c, right) > max(a, x) and min(d, bottom) > max(b, y)


def layout_checks(window, accessibility):
    if not accessibility.get("available"):
        raise RuntimeError(accessibility.get("reason", "Native accessibility is unavailable"))
    nodes = accessibility.get("nodes", [])
    frames = [node for node in nodes if node["role"] == "frame" and node.get("bounds")]
    # GTK's frame includes the title bar; the direct child is its client area.
    clients = [node for node in nodes if node.get("depth") == 2 and node.get("bounds")
               and abs(node["bounds"]["width"] - window["width"]) <= 2
               and abs(node["bounds"]["height"] - window["height"]) <= 2]
    if len(frames) != 1 or len(clients) != 1:
        raise RuntimeError("Could not identify this native window's frame and allocated client area")
    frame, client = frames[0]["bounds"], clients[0]["bounds"]
    found = {name: [node for node in nodes if "button" in node["role"] and node["name"] == name]
             for name in CONTROLS}
    checks = {"sevenNativeButtons": all(len(items) == 1 for items in found.values())}
    if not checks["sevenNativeButtons"]:
        return {"checks": checks, "client": client, "frame": frame, "nodes": nodes}
    buttons = {name: found[name][0].get("bounds", {}) for name in CONTROLS}
    if any(not box for box in buttons.values()):
        raise RuntimeError("A native control has no allocated accessibility bounds")
    compact = window["width"] < 640 or window["height"] < 460
    boxes = list(buttons.values())
    checks.update({
        "buttonsInsideFrame": all(contains(frame, box) for box in boxes),
        "buttonsInsideClient": all(contains(client, box) for box in boxes),
        "buttonsDoNotOverlap": all(not overlaps(a, b) for a, b in itertools.combinations(boxes, 2)),
        "minimumButtonSize": all(box["width"] >= (36 if compact else 44)
                                 and box["height"] >= (36 if compact else 44) for box in boxes),
    })
    axis, cross = ("x", "y") if compact else ("y", "x")
    dimension = "height" if compact else "width"
    aligned = [box[cross] + box[dimension] / 2 for box in boxes]
    service_boxes = [buttons[name] for name in SERVICES]
    checks["horizontalToolbar" if compact else "verticalSidebar"] = max(aligned) - min(aligned) <= 2 and all(
        first[axis] < second[axis] for first, second in zip(service_boxes, service_boxes[1:]))
    content = next((node["bounds"] for node in nodes if node.get("name") == "Content"
                    and node.get("bounds")), None)
    if content is None:
        content = next((node["bounds"] for node in nodes if node["role"] == "label"
                        and node["name"] == "Select an app" and node.get("bounds")), None)
    if content is None:
        raise RuntimeError("The empty native content panel was not exposed")
    x, y, right, bottom = edges(client)
    cx, cy, cr, cb = edges(content)
    checks["contentInsideClient"] = contains(client, content)
    checks["contentDoesNotOverlapButtons"] = all(not overlaps(content, box) for box in boxes)
    if compact:
        toolbar_top = min(box["y"] for box in boxes)
        checks["contentFillsRemainingArea"] = (abs(cx - x) <= 2 and abs(cr - right) <= 2
            and abs(cy - y) <= 2 and 0 <= toolbar_top - cb <= 24
            and content["height"] >= client["height"] - 72)
    else:
        rail_right = max(box["x"] + box["width"] for box in boxes)
        checks["contentFillsRemainingArea"] = (abs(cy - y) <= 2 and abs(cb - bottom) <= 2
            and abs(cr - right) <= 2 and 0 <= cx - rail_right <= 24
            and content["width"] >= client["width"] - 80)
    return {"mode": "compact" if compact else "wide", "checks": checks,
            "client": client, "frame": frame, "buttons": buttons, "content": content}


def resized_window(pid, window_id, width, height, process):
    subprocess.run(["wmctrl", "-ir", window_id, "-e", f"0,80,80,{width},{height}"], check=True)
    deadline, stable = time.monotonic() + 8, 0
    current = None
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError("Native app exited while resizing")
        current = next((item for item in owned_windows(pid) if item["id"] == window_id), None)
        if current and abs(current["width"] - width) <= 2 and abs(current["height"] - height) <= 2:
            stable += 1
            if stable >= 3:
                return current
        else:
            stable = 0
        time.sleep(0.1)
    actual = f'{current["width"]}x{current["height"]}' if current else "missing"
    raise RuntimeError(f"Requested {width}x{height}, but own window remained {actual}")


def smoke(binary, output):
    data = Path(tempfile.mkdtemp(prefix="data-", dir=output))
    os.chmod(data, 0o700)
    command = [str(binary), "--data-dir", str(data)]
    observed, process = {}, None
    report = {"command": command, "privateDataDir": str(data), "checks": {}, "layouts": []}

    def observe():
        current = process_tree(process.pid)
        for item in current:
            observed[(item["pid"], item["startTime"])] = item
        return current

    with (output / "app.log").open("w+") as log:
        try:
            process = subprocess.Popen(command, stdout=log, stderr=log, start_new_session=True,
                                       env=dict(os.environ, GDK_BACKEND="x11"))
            deadline = time.monotonic() + 15
            window = None
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise RuntimeError(f"App exited before showing its window ({process.returncode})")
                observe()
                matches = owned_windows(process.pid)
                if matches:
                    window = matches[0]
                    break
                time.sleep(0.1)
            if window is None:
                raise RuntimeError("No window owned by the launched PID appeared")
            report["checks"]["nativeWindowClass"] = "relay-native" in window["class"].lower()
            subprocess.run(["wmctrl", "-ir", window["id"], "-b", "remove,maximized_vert,maximized_horz"], check=True)
            subprocess.run(["wmctrl", "-ia", window["id"]], check=True)
            for name, width, height in SIZES:
                current = resized_window(process.pid, window["id"], width, height, process)
                observe()
                layout = layout_checks(current, inspect_accessibility(process.pid))
                layout.update({"name": name, "requested": {"width": width, "height": height},
                               "window": current,
                               "screenshot": capture_window(current, output / f"{name}.png")})
                layout["checks"]["screenshotMatchesClient"] = (layout["screenshot"]["width"] == current["width"]
                    and layout["screenshot"]["height"] == current["height"])
                report["layouts"].append(layout)
                report["checks"][f"{name}Layout"] = all(layout["checks"].values())
            report["processesBeforeClose"] = observe()
            report["checks"]["emptyAppStartsNoRenderer"] = not any(map(renderer, observed.values()))
            subprocess.run(["wmctrl", "-ic", window["id"]], check=True)
            try:
                report["exitCode"] = process.wait(timeout=10)
            except subprocess.TimeoutExpired as error:
                raise RuntimeError("App remained running after its own window closed") from error
            report["checks"]["normalWindowCloseExits"] = report["exitCode"] == 0
            deadline = time.monotonic() + 6
            survivors = list(observed.values())
            while time.monotonic() < deadline:
                survivors = [item for item in survivors if same_process(item)]
                if not survivors:
                    break
                time.sleep(0.1)
            report["checks"]["observedChildrenExit"] = not survivors
            report["remainingProcesses"] = survivors
            report["ok"] = all(report["checks"].values())
        except (RuntimeError, OSError, ValueError, subprocess.SubprocessError) as error:
            report.update(ok=False, error=str(error))
        finally:
            if process and process.poll() is None:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait(timeout=5)
            log.flush()
            log.seek(0)
            report["logTail"] = log.read()[-6000:]
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path(__file__).resolve().parents[1] / "bin" / "relay")
    parser.add_argument("--output-dir", type=Path)
    args = parser.parse_args()
    if not shutil.which("wmctrl"):
        parser.error("wmctrl is required for ownership-checked native window resizing")
    binary = args.binary.resolve(strict=True)
    with binary.open("rb") as executable:
        if executable.read(4) != b"\x7fELF":
            parser.error("--binary must identify the native ELF executable")
    output = args.output_dir.resolve() if args.output_dir else Path(tempfile.mkdtemp(prefix="relay-responsive-qa-"))
    output.mkdir(parents=True, exist_ok=True)
    report = smoke(binary, output)
    destination = output / "report.json"
    destination.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"ok": report["ok"], "report": str(destination),
                      "checks": report["checks"], "error": report.get("error")}, indent=2))
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"Responsive native smoke check failed: {error}", file=sys.stderr)
        sys.exit(1)
