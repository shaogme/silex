from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import threading
import time
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


WORKSPACE = Path(__file__).resolve().parents[2]


def load_env() -> None:
    try:
        from dotenv import load_dotenv as _load_dotenv

        _load_dotenv(WORKSPACE / ".env")
    except ImportError:
        dotenv_path = WORKSPACE / ".env"
        if not dotenv_path.is_file():
            return
        with dotenv_path.open(encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                if "=" in line:
                    key, val = line.split("=", 1)
                    key = key.strip()
                    val = val.strip()
                    if (val.startswith('"') and val.endswith('"')) or (
                        val.startswith("'") and val.endswith("'")
                    ):
                        val = val[1:-1]
                    if key and key not in os.environ:
                        os.environ[key] = val


load_env()


def resolve_path(val: str | Path | None, default: Path) -> Path:
    if not val:
        return default
    p = Path(val)
    if not p.is_absolute():
        p = (WORKSPACE / p).resolve()
    return p


PROBE_SCRIPT = """
<script>
(function () {
    const state = { console: [], errors: [], rejections: [], dom: [] };
    window.__counter_probe = state;
    const stringify = value => {
        if (typeof value === "string") return value;
        try { return JSON.stringify(value); } catch (_) { return String(value); }
    };
    const describe = node => {
        if (!node) return null;
        const text = (node.textContent || "").slice(0, 80);
        return { type: node.nodeType, name: node.nodeName, id: node.id || "", text };
    };
    for (const name of ["error", "warn"]) {
        const original = console[name];
        console[name] = function (...args) {
            state.console.push({
                method: name,
                args: args.map(stringify)
            });
            return original.apply(console, args);
        };
    }
    window.addEventListener("error", event => {
        state.errors.push({
            message: event.message,
            filename: event.filename,
            line: event.lineno,
            column: event.colno,
            stack: event.error && event.error.stack
        });
    });
    window.addEventListener("unhandledrejection", event => {
        state.rejections.push(stringify(event.reason));
    });
    const appendChild = Node.prototype.appendChild;
    Node.prototype.appendChild = function (child) {
        state.dom.push({ op: "appendChild", parent: describe(this), child: describe(child) });
        return appendChild.call(this, child);
    };
    const insertBefore = Node.prototype.insertBefore;
    Node.prototype.insertBefore = function (child, reference) {
        state.dom.push({
            op: "insertBefore",
            parent: describe(this),
            child: describe(child),
            reference: describe(reference)
        });
        return insertBefore.call(this, child, reference);
    };
    const removeChild = Node.prototype.removeChild;
    Node.prototype.removeChild = function (child) {
        state.dom.push({ op: "removeChild", parent: describe(this), child: describe(child) });
        return removeChild.call(this, child);
    };
    const createComment = Document.prototype.createComment;
    Document.prototype.createComment = function (data) {
        state.dom.push({ op: "createComment", data });
        return createComment.call(this, data);
    };
})();
</script>
"""


def request_json(method: str, url: str, payload: object | None = None) -> dict:
    body = None if payload is None else json.dumps(payload).encode("utf-8")
    request = Request(
        url,
        data=body,
        method=method,
        headers={"Content-Type": "application/json"} if body else {},
    )
    with urlopen(request, timeout=10) as response:
        data = response.read()
    return json.loads(data.decode("utf-8"))


def wait_for_driver(base_url: str) -> None:
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        try:
            request_json("GET", f"{base_url}/status")
            return
        except (HTTPError, URLError, TimeoutError):
            time.sleep(0.2)
    raise RuntimeError("geckodriver did not become ready")


def make_handler(dist: Path) -> type[SimpleHTTPRequestHandler]:
    class Handler(SimpleHTTPRequestHandler):
        def __init__(self, *args: object, **kwargs: object) -> None:
            super().__init__(*args, directory=str(dist), **kwargs)

        def do_GET(self) -> None:
            path = self.path.split("?", 1)[0]
            if path in {"/", "/index.html"}:
                html = (dist / "index.html").read_text(encoding="utf-8")
                html = html.replace("<head>", f"<head>{PROBE_SCRIPT}", 1)
                body = html.encode("utf-8")
                self.send_response(200)
                self.send_header("Content-Type", "text/html; charset=utf-8")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return
            super().do_GET()

        def log_message(self, format: str, *args: object) -> None:
            return

    return Handler


def execute_script(
    driver_url: str,
    session_id: str,
    script: str,
    args: list[object] | None = None,
) -> object:
    response = request_json(
        "POST",
        f"{driver_url}/session/{session_id}/execute/sync",
        {"script": script, "args": [] if args is None else args},
    )
    return response["value"]


def execute_probe(driver_url: str, session_id: str) -> dict:
    script = """
    const app = document.getElementById("app");
    const comments = app
        ? Array.from(app.querySelectorAll("*"))
            .flatMap(node => Array.from(node.childNodes))
            .filter(node => node.nodeType === Node.COMMENT_NODE)
            .map(node => node.nodeValue)
        : [];
    return {
        href: location.href,
        ready: document.readyState,
        bodyText: document.body.innerText,
        appText: app && app.innerText,
        appHtml: app && app.innerHTML,
        buttons: app
            ? Array.from(app.querySelectorAll("button")).map(node => node.innerText)
            : [],
        links: app
            ? Array.from(app.querySelectorAll("a")).map(node => node.innerText)
            : [],
        inputs: app
            ? Array.from(app.querySelectorAll("input")).map(node => node.value)
            : [],
        comments,
        probe: window.__counter_probe
    };
    """
    result = execute_script(driver_url, session_id, script)
    if not isinstance(result, dict):
        raise RuntimeError("browser probe returned a non-object snapshot")
    return result


def require_text(snapshot: dict, expected: list[str], phase: str) -> None:
    text = snapshot.get("appText") or ""
    missing = [value for value in expected if value not in text]
    if missing:
        raise RuntimeError(f"{phase} is missing {missing}: {text!r}")


def require_no_browser_errors(snapshot: dict, phase: str) -> None:
    probe = snapshot.get("probe") or {}
    errors = probe.get("errors") or []
    rejections = probe.get("rejections") or []
    console_failures = [
        entry for entry in probe.get("console", []) if entry.get("method") in {"error", "warn"}
    ]
    if errors or rejections or console_failures:
        raise RuntimeError(
            f"{phase} reported browser failures: errors={errors!r}, "
            f"rejections={rejections!r}, console={console_failures!r}"
        )


def click_button(driver_url: str, session_id: str, label: str) -> None:
    execute_script(
        driver_url,
        session_id,
        """
        const button = Array.from(document.querySelectorAll("#app button"))
            .find(node => node.innerText.trim() === arguments[0]);
        if (!button) throw new Error(`button ${arguments[0]} was not found`);
        button.click();
        return true;
        """,
        [label],
    )


def update_name(driver_url: str, session_id: str, name: str) -> None:
    execute_script(
        driver_url,
        session_id,
        """
        const input = document.querySelector("#app input");
        if (!input) throw new Error("counter input was not found");
        input.value = arguments[0];
        input.dispatchEvent(new Event("input", { bubbles: true }));
        return true;
        """,
        [name],
    )


def click_link(driver_url: str, session_id: str, label: str) -> None:
    execute_script(
        driver_url,
        session_id,
        """
        const link = Array.from(document.querySelectorAll("#app a"))
            .find(node => node.innerText.trim() === arguments[0]);
        if (!link) throw new Error(`link ${arguments[0]} was not found`);
        link.click();
        return true;
        """,
        [label],
    )


def unmount_counter(driver_url: str, session_id: str) -> None:
    execute_script(
        driver_url,
        session_id,
        """
        window.dispatchEvent(new Event("counter-unmount"));
        return true;
        """,
    )


def run(args: argparse.Namespace) -> int:
    dist = args.dist.resolve()
    if not (dist / "index.html").is_file():
        raise FileNotFoundError(f"missing generated app: {dist / 'index.html'}")
    if not args.geckodriver.is_file():
        raise FileNotFoundError(f"missing geckodriver: {args.geckodriver}")
    if not args.firefox.is_file():
        raise FileNotFoundError(f"missing Firefox: {args.firefox}")

    server = ThreadingHTTPServer(
        ("127.0.0.1", args.http_port),
        make_handler(dist),
    )
    server_thread = threading.Thread(target=server.serve_forever, daemon=True)
    server_thread.start()

    driver = subprocess.Popen(
        [str(args.geckodriver), "--port", str(args.driver_port)],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    driver_url = f"http://127.0.0.1:{args.driver_port}"
    session_id: str | None = None
    try:
        wait_for_driver(driver_url)
        capabilities = {
            "capabilities": {
                "alwaysMatch": {
                    "browserName": "firefox",
                    "moz:firefoxOptions": {
                        "binary": str(args.firefox),
                        "args": ["-headless"],
                    },
                }
            }
        }
        session = request_json("POST", f"{driver_url}/session", capabilities)
        session_id = session["value"]["sessionId"]
        request_json(
            "POST",
            f"{driver_url}/session/{session_id}/url",
            {"url": f"http://127.0.0.1:{args.http_port}/"},
        )
        time.sleep(args.wait)
        initial = execute_probe(driver_url, session_id)
        require_text(
            initial,
            [
                "Silex: Next Gen",
                "Explicit Counter",
                "Local State (Resets on Nav)",
                "Control Flow",
                "Suspense (Async Loading)",
            ],
            "initial counter page",
        )
        require_no_browser_errors(initial, "initial counter page")

        click_button(driver_url, session_id, "+")
        time.sleep(0.25)
        after_count = execute_probe(driver_url, session_id)
        require_text(after_count, ["1"], "counter increment")

        update_name(driver_url, session_id, "Ada")
        time.sleep(0.25)
        after_name = execute_probe(driver_url, session_id)
        require_text(after_name, ["Hello, Ada!"], "name update")

        time.sleep(2.25)
        after_suspense = execute_probe(driver_url, session_id)
        require_text(after_suspense, ["Loaded Data from Server!"], "suspense completion")

        click_link(driver_url, session_id, "About")
        time.sleep(0.25)
        after_about = execute_probe(driver_url, session_id)
        require_text(after_about, ["This is the About Page"], "About navigation")

        click_link(driver_url, session_id, "Home")
        time.sleep(0.25)
        after_home = execute_probe(driver_url, session_id)
        require_text(after_home, ["Silex: Next Gen"], "Home navigation")

        unmount_counter(driver_url, session_id)
        time.sleep(0.25)
        after_unmount = execute_probe(driver_url, session_id)
        if after_unmount.get("appText") not in ("", None):
            raise RuntimeError(
                f"explicit counter unmount left app content: {after_unmount.get('appText')!r}"
            )
        if after_unmount.get("appHtml") not in ("", None):
            raise RuntimeError(
                f"explicit counter unmount left app HTML: {after_unmount.get('appHtml')!r}"
            )
        if after_unmount.get("comments"):
            raise RuntimeError(
                f"explicit counter unmount left framework comments: {after_unmount.get('comments')!r}"
            )

        for phase, snapshot in [
            ("initial counter page", initial),
            ("counter increment", after_count),
            ("name update", after_name),
            ("suspense completion", after_suspense),
            ("About navigation", after_about),
            ("Home navigation", after_home),
            ("explicit counter unmount", after_unmount),
        ]:
            require_no_browser_errors(snapshot, phase)

        print(
            json.dumps(
                {
                    "initial": initial,
                    "afterCount": after_count,
                    "afterName": after_name,
                    "afterSuspense": after_suspense,
                    "afterAbout": after_about,
                    "afterHome": after_home,
                    "afterUnmount": after_unmount,
                },
                indent=2,
                ensure_ascii=True,
            )
        )
        return 0
    finally:
        if session_id is not None:
            try:
                request_json("DELETE", f"{driver_url}/session/{session_id}")
            except (HTTPError, URLError, TimeoutError):
                pass
        server.shutdown()
        server.server_close()
        driver.terminate()
        try:
            driver.wait(timeout=5)
        except subprocess.TimeoutExpired:
            driver.kill()
            driver.wait()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Probe the counter example in Firefox.")

    env_dist = os.environ.get("COUNTER_DIST")
    default_dist = resolve_path(env_dist, WORKSPACE / "examples" / "counter" / "dist")

    env_geckodriver = os.environ.get("GECKODRIVER") or os.environ.get("GECKODRIVER_PATH")
    default_geckodriver = resolve_path(
        env_geckodriver, WORKSPACE / "tools" / "geckodriver" / "geckodriver.exe"
    )

    env_firefox = os.environ.get("FIREFOX_BINARY") or os.environ.get("FIREFOX")
    default_firefox = resolve_path(
        env_firefox, Path(r"C:\Program Files\Firefox Developer Edition\firefox.exe")
    )

    default_http_port = int(
        os.environ.get("COUNTER_HTTP_PORT") or os.environ.get("HTTP_PORT") or 8087
    )
    default_driver_port = int(
        os.environ.get("COUNTER_DRIVER_PORT") or os.environ.get("DRIVER_PORT") or 4444
    )
    default_wait = float(
        os.environ.get("COUNTER_PROBE_WAIT") or os.environ.get("WAIT") or 4.0
    )

    parser.add_argument("--dist", type=Path, default=default_dist)
    parser.add_argument("--geckodriver", type=Path, default=default_geckodriver)
    parser.add_argument("--firefox", type=Path, default=default_firefox)
    parser.add_argument("--http-port", type=int, default=default_http_port)
    parser.add_argument("--driver-port", type=int, default=default_driver_port)
    parser.add_argument("--wait", type=float, default=default_wait)
    return parser.parse_args()


if __name__ == "__main__":
    try:
        raise SystemExit(run(parse_args()))
    except (FileNotFoundError, RuntimeError, HTTPError, URLError) as error:
        print(f"probe failed: {error}", file=sys.stderr)
        raise SystemExit(1)
