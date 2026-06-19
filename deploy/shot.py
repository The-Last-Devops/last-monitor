#!/usr/bin/env python3
"""Screenshot a (possibly authenticated) hub page via headless Chrome + CDP.

Usage: shot.py <url> <out.png> [email] [password]

Logs in over HTTP to obtain the session cookie, drives Chrome through the
DevTools Protocol (stdlib websocket, no deps), and writes a PNG.
"""
import base64, http.client, json, os, socket, struct, subprocess, sys, time, urllib.request, urllib.error

CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
PORT = 9222
HOST = "127.0.0.1"


def login(base, email, password):
    body = json.dumps({"email": email, "password": password}).encode()
    req = urllib.request.Request(base + "/api/auth/login", data=body,
                                 headers={"content-type": "application/json"})
    try:
        resp = urllib.request.urlopen(req, timeout=5)
    except urllib.error.HTTPError as e:
        print("login failed", e.code); return None
    for h, v in resp.getheaders():
        if h.lower() == "set-cookie" and v.startswith("session="):
            return v.split(";")[0].split("=", 1)[1]
    return None


# --- minimal RFC6455 client ---
def ws_connect(url):
    # ws://127.0.0.1:9222/devtools/page/XXXX
    path = url.split(PORT and f"{HOST}:{PORT}")[1]
    s = socket.create_connection((HOST, PORT))
    key = base64.b64encode(os.urandom(16)).decode()
    s.send(("GET %s HTTP/1.1\r\nHost: %s:%d\r\nUpgrade: websocket\r\n"
            "Connection: Upgrade\r\nSec-WebSocket-Key: %s\r\n"
            "Sec-WebSocket-Version: 13\r\n\r\n" % (path, HOST, PORT, key)).encode())
    buf = b""
    while b"\r\n\r\n" not in buf:
        buf += s.recv(4096)
    return s


def ws_send(s, msg):
    data = msg.encode()
    hdr = bytearray([0x81])  # FIN + text
    n = len(data)
    mask = os.urandom(4)
    if n < 126:
        hdr.append(0x80 | n)
    elif n < 65536:
        hdr.append(0x80 | 126); hdr += struct.pack(">H", n)
    else:
        hdr.append(0x80 | 127); hdr += struct.pack(">Q", n)
    hdr += mask
    s.send(bytes(hdr) + bytes(b ^ mask[i % 4] for i, b in enumerate(data)))


def _read(s, n):
    out = b""
    while len(out) < n:
        chunk = s.recv(n - len(out))
        if not chunk:
            raise IOError("closed")
        out += chunk
    return out


def ws_recv(s):
    payload = b""
    while True:
        b0, b1 = _read(s, 2)
        fin = b0 & 0x80
        ln = b1 & 0x7F
        if ln == 126:
            ln = struct.unpack(">H", _read(s, 2))[0]
        elif ln == 127:
            ln = struct.unpack(">Q", _read(s, 8))[0]
        payload += _read(s, ln)
        if fin:
            break
    return payload.decode("utf-8", "replace")


def cmd(s, _id, method, params=None):
    ws_send(s, json.dumps({"id": _id, "method": method, "params": params or {}}))
    while True:
        msg = json.loads(ws_recv(s))
        if msg.get("id") == _id:
            return msg.get("result", {})


def main():
    url, out = sys.argv[1], sys.argv[2]
    email = sys.argv[3] if len(sys.argv) > 3 else None
    password = sys.argv[4] if len(sys.argv) > 4 else None
    base = "http://" + url.split("//")[1].split("/")[0]

    cookie = login(base, email, password) if email else None

    proc = subprocess.Popen(
        [CHROME, "--headless=new", "--disable-gpu", "--no-sandbox",
         f"--remote-debugging-port={PORT}", "--window-size=1440,1100",
         "--hide-scrollbars", "about:blank"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        ws_url = None
        for _ in range(50):
            try:
                c = http.client.HTTPConnection(HOST, PORT, timeout=1)
                c.request("GET", "/json")
                targets = json.loads(c.getresponse().read())
                for t in targets:
                    if t.get("type") == "page":
                        ws_url = t["webSocketDebuggerUrl"]; break
                if ws_url:
                    break
            except Exception:
                time.sleep(0.2)
        if not ws_url:
            print("no chrome target"); return 1

        s = ws_connect(ws_url)
        i = 0
        def nxt():
            nonlocal i; i += 1; return i
        cmd(s, nxt(), "Network.enable")
        if cookie:
            cmd(s, nxt(), "Network.setCookie",
                {"name": "session", "value": cookie, "url": base})
        cmd(s, nxt(), "Page.enable")
        cmd(s, nxt(), "Page.navigate", {"url": url})
        time.sleep(2.5)  # let HTMX fragments + charts render
        # Optional hover: pass HOVER=x,y env to simulate a mousemove before capture.
        ev = os.environ.get("EVAL")
        if ev:
            cmd(s, nxt(), "Runtime.evaluate", {"expression": ev})
            time.sleep(0.6)
        hover = os.environ.get("HOVER")
        if hover:
            hx, hy = (float(v) for v in hover.split(","))
            cmd(s, nxt(), "Input.dispatchMouseEvent",
                {"type": "mouseMoved", "x": hx, "y": hy})
            time.sleep(0.4)
        res = cmd(s, nxt(), "Page.captureScreenshot", {"format": "png"})
        with open(out, "wb") as f:
            f.write(base64.b64decode(res["data"]))
        print("wrote", out)
        return 0
    finally:
        proc.terminate()


if __name__ == "__main__":
    sys.exit(main())
