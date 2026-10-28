#!/usr/bin/env python3
"""Independent Linux/X11 smoke check for Relay's native GTK/WebKit executable.

Launches a fresh private data directory, inspects only its own PID/window,
captures only that window, requests a normal window close, and checks that the
observed child processes exit. No browser profile, extension, account login, or
message sending is involved. Requires wmctrl and Python GI's Gdk/GdkX11.

Examples:
  python3 tests/native_smoke.py --output-dir /tmp/relay-native-qa
  python3 tests/native_smoke.py --service telegram --output-dir /tmp/relay-telegram-qa
  python3 tests/native_smoke.py --run-self-test --output-dir /tmp/relay-fixture-qa
"""

import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time


def process_info(pid, details=True):
    try:
        base = Path("/proc") / str(pid)
        stat = (base / "stat").read_text()
        end = stat.rfind(")")
        fields = stat[end + 2:].split()
        basic = {"pid": pid, "parentPid": int(fields[1]), "state": fields[0], "startTime": fields[19]}
        if not details:
            return basic
        command = (base / "cmdline").read_bytes().replace(b"\0", b" ").decode(errors="replace").strip()
        executable = os.readlink(base / "exe")
        memory = {}
        try:
            for line in (base / "smaps_rollup").read_text().splitlines():
                parts = line.split()
                if parts and parts[0] in ("Rss:", "Pss:"):
                    memory[parts[0][:-1].lower() + "Bytes"] = int(parts[1]) * 1024
        except (OSError, ValueError):
            pass
        return {**basic, "executable": executable,
                "command": command, **memory}
    except (OSError, ValueError, IndexError):
        return None


def process_tree(pid):
    table = {}
    for entry in Path("/proc").iterdir():
        if entry.name.isdigit():
            item = process_info(int(entry.name), details=False)
            if item:
                table[item["pid"]] = item
    selected = {pid}
    while True:
        more = {item["pid"] for item in table.values() if item["parentPid"] in selected}
        if more.issubset(selected):
            return [info for item in sorted(selected) if item in table and (info := process_info(item))]
        selected.update(more)


def renderer(item):
    return "WebKitWebProcess" in Path(item["executable"]).name


def owned_windows(pid):
    result = subprocess.run(["wmctrl", "-lpGx"], check=True, capture_output=True, text=True)
    windows = []
    for line in result.stdout.splitlines():
        parts = line.split(None, 9)
        if len(parts) < 9 or not parts[2].isdigit() or int(parts[2]) != pid:
            continue
        windows.append({"id": parts[0], "pid": pid, "class": parts[7],
                        "x": int(parts[3]), "y": int(parts[4]),
                        "width": int(parts[5]), "height": int(parts[6]),
                        "title": parts[9] if len(parts) > 9 else ""})
    return windows


def capture_window(window, destination):
    import gi
    gi.require_version("Gdk", "3.0")
    gi.require_version("GdkX11", "3.0")
    from gi.repository import Gdk, GdkX11
    Gdk.init([])
    display = Gdk.Display.get_default()
    if display is None:
        raise RuntimeError("No X11 display is available for a native GUI check")
    foreign = GdkX11.X11Window.foreign_new_for_display(display, int(window["id"], 16))
    if foreign is None:
        raise RuntimeError("The application's X11 window disappeared")
    # Foreign GDK windows cache their first allocation across later X11 resizes.
    # Use the freshly measured client rectangle to capture only this window.
    width, height = window["width"], window["height"]
    display.sync()
    pixbuf = Gdk.pixbuf_get_from_window(foreign, 0, 0, width, height)
    if pixbuf is None:
        raise RuntimeError("GDK could not capture the application's own window")
    pixbuf.savev(str(destination), "png", [], [])
    return {"path": str(destination), "width": width, "height": height,
            "sourceWindow": window["id"], "sourcePid": window["pid"]}


def accessibility_probe(pid, action_target=None):
    # AT-SPI is cross-process; select the application by PID before reading any
    # descendant names. Never report another application's accessibility tree.
    import gi
    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi
    Atspi.init()
    desktop = Atspi.get_desktop(0)
    application = None
    for index in range(desktop.get_child_count()):
        candidate = desktop.get_child_at_index(index)
        if candidate and candidate.get_process_id() == pid:
            application = candidate
            break
    if application is None:
        return {"available": False, "reason": "Application is not exposed on AT-SPI"}
    nodes = []
    performed = None

    def walk(node, depth):
        nonlocal performed
        if node is None or depth > 12 or len(nodes) >= 150:
            return
        item = {"role": node.get_role_name(), "name": node.get_name(), "depth": depth}
        if hasattr(node, "get_accessible_id"):
            item["accessibleId"] = node.get_accessible_id()
        try:
            component = node.get_component_iface()
            if component:
                box = component.get_extents(Atspi.CoordType.SCREEN)
                item["bounds"] = {"x": box.x, "y": box.y, "width": box.width, "height": box.height}
        except Exception:
            pass
        nodes.append(item)
        if action_target and performed is None and item["name"] == action_target and "button" in item["role"]:
            action = node.get_action_iface()
            if action and action.get_n_actions():
                performed = {"target": action_target, "action": action.get_action_name(0),
                             "invoked": bool(action.do_action(0))}
        for index in range(node.get_child_count()):
            walk(node.get_child_at_index(index), depth + 1)

    walk(application, 0)
    return {"available": True, "pid": pid, "nodes": nodes, "performedAction": performed}


def inspect_accessibility(pid, action_target=None):
    try:
        command = [sys.executable, str(Path(__file__).resolve()), "--atspi-pid", str(pid)]
        if action_target:
            command.extend(["--atspi-action", action_target])
        result = subprocess.run(command,
                                text=True, capture_output=True, timeout=5)
        if result.returncode:
            return {"available": False, "reason": result.stderr.strip()[-800:] or "AT-SPI probe failed"}
        return json.loads(result.stdout)
    except (subprocess.TimeoutExpired, ValueError):
        return {"available": False, "reason": "AT-SPI probe unavailable or timed out"}


def same_process(item):
    current = process_info(item["pid"])
    return current is not None and current["startTime"] == item["startTime"] and current["state"] != "Z"


def smoke(binary, output, service, activate_service=None):
    data = Path(tempfile.mkdtemp(prefix="data-", dir=output))
    os.chmod(data, 0o700)
    command = [str(binary), "--data-dir", str(data)]
    if service:
        command.extend(["--open-service", service])
    environment = dict(os.environ, GDK_BACKEND="x11")
    observed = {}
    report = {"command": command, "privateDataDir": str(data), "checks": {}, "service": service}
    process = None
    with (output / "app.log").open("w+") as log:
        try:
            process = subprocess.Popen(command, stdout=log, stderr=log, env=environment,
                                       start_new_session=True)
            deadline = time.monotonic() + 15
            window = None
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise RuntimeError(f"Native executable exited before showing a window ({process.returncode})")
                for item in process_tree(process.pid):
                    observed[(item["pid"], item["startTime"])] = item
                matches = owned_windows(process.pid)
                if matches:
                    window = matches[0]
                    break
                time.sleep(0.1)
            if not window:
                raise RuntimeError("No X11 window owned by the launched PID appeared")
            report["window"] = window
            report["checks"]["windowOwnedByLaunchedPid"] = window["pid"] == process.pid
            report["checks"]["nativeWindowClass"] = "relay-native" in window["class"].lower()
            # Observe an actual idle period to catch eager renderer startup,
            # rather than taking only the first short-lived process snapshot.
            deadline = time.monotonic() + (12 if service else 1.5)
            sample_started = time.monotonic()
            samples = []
            while time.monotonic() < deadline:
                current = process_tree(process.pid)
                for item in current:
                    observed[(item["pid"], item["startTime"])] = item
                samples.append({"elapsed": round(time.monotonic() - sample_started, 3), "renderers": sum(map(renderer, current))})
                if service and any(map(renderer, current)):
                    time.sleep(0.7)
                    break
                time.sleep(0.15)
            current = process_tree(process.pid)
            root = next((item for item in current if item["pid"] == process.pid), None)
            if root is None:
                raise RuntimeError("The launched native process disappeared")
            report["checks"]["nativeExecutableStaysRunning"] = Path(root["executable"]).resolve() == binary
            maps = Path(f"/proc/{process.pid}/maps").read_text()
            report["checks"]["gtk3Loaded"] = "libgtk-3.so" in maps
            report["checks"]["webkitGtk41Loaded"] = "libwebkit2gtk-4.1.so" in maps
            report["checks"]["noFirefoxEngine"] = "libxul.so" not in maps and all(
                "firefox" not in Path(item["executable"]).name.lower() for item in observed.values())
            saw_renderer = any(renderer(item) for item in observed.values())
            report["checks"]["rendererIsLazy" if not service else "serviceCreatesRenderer"] = not saw_renderer if not service else saw_renderer
            report["processes"] = current
            report["rendererSamples"] = samples
            report["accessibility"] = inspect_accessibility(process.pid)
            if report["accessibility"].get("available"):
                button_names = {node["name"] for node in report["accessibility"]["nodes"] if "button" in node["role"]}
                report["checks"]["nativeServiceAccessibleNames"] = {"WhatsApp", "Telegram", "Instagram", "LinkedIn", "Gmail"}.issubset(button_names)
            # wmctrl targets only the window already verified as this PID's.
            subprocess.run(["wmctrl", "-ia", window["id"]], check=True)
            time.sleep(0.25)
            if activate_service:
                report["initialScreenshot"] = capture_window(window, output / "initial-window.png")
                names = {"whatsapp": "WhatsApp", "telegram": "Telegram", "instagram": "Instagram", "linkedin": "LinkedIn", "gmail": "Gmail"}
                activated = inspect_accessibility(process.pid, names[activate_service])
                report["nativeActivation"] = activated.get("performedAction")
                if not (activated.get("performedAction") or {}).get("invoked"):
                    raise RuntimeError("The native accessible service button could not be activated")
                deadline = time.monotonic() + 12
                activated_renderer = False
                while time.monotonic() < deadline:
                    current = process_tree(process.pid)
                    for item in current:
                        observed[(item["pid"], item["startTime"])] = item
                    if any(map(renderer, current)):
                        activated_renderer = True
                        time.sleep(0.8)
                        break
                    time.sleep(0.1)
                report["checks"]["nativeButtonCreatesRenderer"] = activated_renderer
                report["processesAfterActivation"] = process_tree(process.pid)
            report["screenshot"] = capture_window(window, output / "window.png")
            subprocess.run(["wmctrl", "-ic", window["id"]], check=True)
            try:
                report["exitCode"] = process.wait(timeout=10)
            except subprocess.TimeoutExpired as error:
                raise RuntimeError("Native app remained running after its own window closed") from error
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
        except (RuntimeError, OSError, subprocess.SubprocessError) as error:
            report["ok"] = False
            report["error"] = str(error)
        finally:
            if process and process.poll() is None:
                # Cleanup is scoped to the process group created by this test.
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


def run_self_test(binary, output):
    data = Path(tempfile.mkdtemp(prefix="fixture-data-", dir=output))
    report_path = output / "self-test.json"
    snapshot = output / "fixture.png"
    command = [str(binary), "--data-dir", str(data), "--self-test", str(report_path), "--snapshot", str(snapshot)]
    process = subprocess.Popen(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               start_new_session=True, env=dict(os.environ, GDK_BACKEND="x11"))
    try:
        _, stderr = process.communicate(timeout=120)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.communicate(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.communicate(timeout=5)
        raise RuntimeError("Native fixture self-test timed out")
    if process.returncode != 0:
        raise RuntimeError("Native fixture self-test failed: " + stderr[-4000:])
    report = json.loads(report_path.read_text())
    facts = dict(report)
    if isinstance(report.get("checks"), list):
        for result in report["checks"]:
            if isinstance(result, dict):
                facts.update(result)
    if facts.get("initialWebViews") != 0:
        raise RuntimeError("Fixture did not confirm zero initial WebViews")
    widget_type = facts.get("nativeWidgetType", facts.get("windowWidget", ""))
    if not isinstance(widget_type, str) or not widget_type.startswith("Gtk"):
        raise RuntimeError("Fixture did not identify a native GTK shell type")
    if facts.get("buttonWidget") != "GtkButton" or facts.get("viewWidget") != "WebKitWebView":
        raise RuntimeError("Fixture did not identify native GTK buttons and an embedded WebKit view")
    for expected in ["inactiveViewDestroyed", "otherViewAlive", "wakeRecreatedNativeView", "sandboxEnabled"]:
        if facts.get(expected) is not True:
            raise RuntimeError("Native fixture did not confirm " + expected)
    for expected in ["retainedAfter35Seconds", "longIdleViewDestroyed", "latestSelectionWins"]:
        if facts.get(expected) is not True:
            raise RuntimeError("Native switching fixture did not confirm " + expected)
    for expected in ["singlePaneNavigation", "whatsappDraftPreserved", "normalZoomThroughout", "spaReplacementRecovered", "resizePreservedLoadedViews"]:
        if facts.get(expected) is not True:
            raise RuntimeError("WhatsApp single-pane fixture did not confirm " + expected)
    if facts.get("singlePaneSteps") != 12 or facts.get("resizeExtraDocumentLoads") != 0:
        raise RuntimeError("WhatsApp single-pane fixture reloaded or skipped navigation")
    if facts.get("warmSwitches") != 40 or facts.get("warmExtraDocumentLoads") != 0:
        raise RuntimeError("Native switching fixture reloaded warm pages")
    if report.get("ok") is False or report.get("passed") is False or report.get("failures") or report.get("errors"):
        raise RuntimeError("Native fixture report contains failed checks")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path(__file__).resolve().parents[1] / "bin" / "relay")
    parser.add_argument("--output-dir", type=Path)
    service_mode = parser.add_mutually_exclusive_group()
    service_mode.add_argument("--service", choices=["whatsapp", "telegram", "instagram", "linkedin", "gmail"])
    service_mode.add_argument("--activate-service", choices=["whatsapp", "telegram", "instagram", "linkedin", "gmail"], help="Click the actual native service button through this application's accessibility interface")
    parser.add_argument("--run-self-test", action="store_true")
    parser.add_argument("--atspi-pid", type=int, help=argparse.SUPPRESS)
    parser.add_argument("--atspi-action", help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.atspi_pid is not None:
        print(json.dumps(accessibility_probe(args.atspi_pid, args.atspi_action)))
        return 0
    if not shutil.which("wmctrl"):
        parser.error("wmctrl is required for ownership-checked native window control")
    binary = args.binary.resolve(strict=True)
    with binary.open("rb") as executable:
        magic = executable.read(4)
    if magic != b"\x7fELF":
        parser.error("--binary must identify the native ELF executable")
    output = args.output_dir.resolve() if args.output_dir else Path(tempfile.mkdtemp(prefix="relay-native-qa-"))
    output.mkdir(parents=True, exist_ok=True)
    report = smoke(binary, output, args.service, args.activate_service)
    if args.run_self_test and report["ok"]:
        try:
            report["fixture"] = run_self_test(binary, output)
        except (RuntimeError, OSError, ValueError, subprocess.SubprocessError) as error:
            report["ok"] = False
            report["fixtureError"] = str(error)
    (output / "report.json").write_text(json.dumps(report, indent=2))
    print(json.dumps({"ok": report["ok"], "report": str(output / "report.json"),
                      "screenshot": report.get("screenshot"), "checks": report["checks"],
                      "error": report.get("error", report.get("fixtureError"))}, indent=2))
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"Native smoke check failed: {error}", file=sys.stderr)
        sys.exit(1)
