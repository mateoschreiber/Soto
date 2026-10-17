#!/usr/bin/env python3
"""Exercise already-built API and worker images with generated, redistributable media."""
import argparse
import gzip
import http.cookiejar
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", default="soto:local")
    parser.add_argument("--worker-image", default="soto:worker-local")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    with tempfile.TemporaryDirectory(prefix="soto-smoke-") as directory:
        media = Path(directory) / "media"
        media.mkdir()
        media.chmod(0o755)
        Path(directory).chmod(0o755)
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", 0))
            port = reservation.getsockname()[1]
        env = dict(os.environ, SOTO_IMAGE=args.image,
                   SOTO_WORKER_IMAGE=args.worker_image, SOTO_LOCAL_PATH=str(media),
                   SOTO_PORT=str(port), SOTO_HOST_IP="127.0.0.1", SOTO_PUBLIC_URL="",
                   SOTO_MEMORY_LIMIT="256m", SOTO_WORKER_MEMORY_LIMIT="1g")
        project = "soto-smoke-" + uuid.uuid4().hex[:10]
        compose = ["docker", "compose", "-p", project, "-f", str(root / "compose.yaml")]

        def run(*command, capture=False):
            return subprocess.run(command, cwd=root, env=env, check=True,
                                  text=True, capture_output=capture, timeout=240)

        def dc(*command, capture=False):
            return run(*compose, *command, capture=capture)

        base = f"http://127.0.0.1:{port}"
        client = urllib.request.build_opener(
            urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))

        def request(path, method="GET", data=None, headers=None, authenticated=True):
            outgoing = dict(headers or {})
            payload = None
            if data is not None:
                payload = json.dumps(data).encode()
                outgoing["Content-Type"] = "application/json"
            if method not in ("GET", "HEAD"):
                outgoing["x-soto-request"] = "1"
            req = urllib.request.Request(base + path, payload, outgoing, method=method)
            opener = client if authenticated else urllib.request.build_opener()
            with opener.open(req, timeout=40) as response:
                return response.status, response.headers, response.read()

        def api(path, method="GET", data=None):
            return json.loads(request("/api/v1" + path, method, data)[2])

        def eventually(operation, predicate, seconds=90):
            deadline = time.monotonic() + seconds
            last = None
            while time.monotonic() < deadline:
                try:
                    last = operation()
                    if predicate(last):
                        return last
                except (urllib.error.URLError, TimeoutError, ConnectionError) as error:
                    last = type(error).__name__
                time.sleep(0.5)
            raise AssertionError(f"Timed out waiting for expected state; last result: {last}")

        try:
            print("Generating six seconds of test video and audio", flush=True)
            run("docker", "run", "--rm", "--user", "0:0", "--entrypoint", "ffmpeg",
                "-v", f"{media}:/media", args.worker_image,
                "-nostdin", "-hide_banner", "-loglevel", "error", "-threads", "1",
                "-f", "lavfi", "-i", "testsrc=size=320x180:rate=24",
                "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000",
                "-t", "6", "-c:v", "mpeg4", "-threads", "1", "-pix_fmt", "yuv420p",
                "-c:a", "aac", "-ac", "2", "/media/Controlled (2026).mp4")
            dc("config", "--quiet")
            dc("up", "-d", "--wait", "--wait-timeout", "120")
            for service, executable in [("web", "soto-server"), ("worker", "soto-worker")]:
                dc("exec", "-T", service, f"/app/{executable}", "--healthcheck")
                assert dc("exec", "-T", service, "id", "-u", capture=True).stdout.strip() == "10001"
                container = dc("ps", "-q", service, capture=True).stdout.strip()
                readonly = run("docker", "inspect", "-f", "{{.HostConfig.ReadonlyRootfs}}", container, capture=True)
                assert readonly.stdout.strip() == "true"
                if service == "worker":
                    bindings = json.loads(run("docker", "inspect", "-f", "{{json .HostConfig.PortBindings}}", container, capture=True).stdout)
                    assert not bindings, "Worker must have no published host port"

            assert api("/setup")["configured"] is False
            token = dc("exec", "-T", "web", "/app/soto-server", "--print-setup-token", capture=True).stdout.strip()
            assert token
            password = "Smoke-" + uuid.uuid4().hex
            api("/setup", "POST", {"setup_token": token, "server_name": "Controlled smoke",
                                   "username": "smoke", "password": password})
            assert api("/session")["admin"] is True
            api("/settings", "PUT", {"server_name": "Controlled smoke", "torrent_enabled": False,
                                     "torrent_quota_gb": 0, "cache_quota_gb": 1,
                                     "download_kib": 0, "upload_kib": 1024})
            try:
                request("/api/v1/catalog", authenticated=False)
                raise AssertionError("Catalog accepted an anonymous request")
            except urllib.error.HTTPError as error:
                assert error.code == 401

            print("Checking compiled interface and compressed WASM", flush=True)
            html = request("/")[2].decode()
            assert "Soto" in html
            javascript = re.search(r'(/\./assets/[^" ]+\.js)', html)
            assert javascript, "Compiled application script missing"
            code = request(javascript.group(1))[2].decode()
            wasm = re.search(r'soto-web_bg-dxh[0-9a-f]+\.wasm', code)
            assert wasm, "Compiled application WASM missing"
            _, headers, compressed = request("/assets/" + wasm.group(), headers={"Accept-Encoding": "gzip"})
            assert headers.get("Content-Encoding") == "gzip"
            assert "accept-encoding" in headers.get("Vary", "").lower()
            assert gzip.decompress(compressed).startswith(b"\x00asm")

            print("Scanning the mounted generated file through the worker", flush=True)
            library = api("/libraries", "POST", {"name": "Controlled media", "root_id": "local", "path": "", "kind": "movie"})
            scan = api(f"/libraries/{library['id']}/scan", "POST", {})
            job = eventually(lambda: next((j for j in api("/jobs") if j["id"] == scan["id"]), None),
                             lambda j: j is not None and j["status"] in ("completed", "failed", "cancelled"))
            assert job["status"] == "completed", job
            catalog = api("/catalog?origin=local")
            assert len(catalog) == 1, catalog
            item = catalog[0]
            details = api("/catalog/" + item["id"])
            source = next(source for source in details["sources"] if source["kind"] == "local")
            direct = api("/playback", "POST", {"source_id": source["id"], "mode": "direct"})
            status, headers, payload = request(direct["url"], headers={"Range": "bytes=0-31"})
            assert status == 206 and len(payload) == 32
            assert headers.get("Content-Range", "").startswith("bytes 0-31/")
            assert payload == (media / "Controlled (2026).mp4").read_bytes()[:32]
            status, _, tail = request(direct["url"], headers={"Range": "bytes=-17"})
            assert status == 206 and tail == (media / "Controlled (2026).mp4").read_bytes()[-17:]
            api("/progress/" + item["id"], "PUT", {"position": 1.25, "duration": 6.0})
            api("/playback/" + direct["id"], "DELETE")

            print("Converting actual media and fetching authenticated fMP4 HLS", flush=True)
            converted = api("/playback", "POST", {"source_id": source["id"], "mode": "convert"})
            assert converted["mode"] == "convert"
            assert converted.get("timeline_offset") == 1.25
            playlist = eventually(lambda: request(converted["url"])[2].decode(),
                                  lambda value: "#EXT-X-MAP:" in value and ".m4s" in value)
            init = re.search(r'#EXT-X-MAP:URI="([^"]+)"', playlist)
            segment = next(line for line in playlist.splitlines() if line and not line.startswith("#") and line.endswith(".m4s"))
            assert init and init.group(1) == "init.mp4"
            for filename, box in [(init.group(1), b"ftyp"), (segment, b"moof")]:
                assert "/" not in filename and ".." not in filename
                path = urllib.parse.urljoin(converted["url"], filename)
                output = request(path)[2]
                assert box in output[:256], f"Missing fMP4 {box!r} box"
            api("/playback/" + converted["id"], "DELETE")

            print("Restarting both services and checking persisted session/catalog/progress", flush=True)
            dc("restart")
            eventually(lambda: api("/session"), lambda value: value.get("username") == "smoke")
            eventually(lambda: subprocess.run([*compose, "exec", "-T", "worker", "/app/soto-worker", "--healthcheck"],
                                              cwd=root, env=env, capture_output=True, timeout=10).returncode,
                       lambda status: status == 0)
            assert api("/setup")["configured"] is True
            assert any(entry["id"] == item["id"] for entry in api("/catalog?origin=local"))
            saved = next(entry for entry in api("/progress") if entry["media_id"] == item["id"])
            assert saved["position"] == 1.25 and saved["duration"] == 6.0
            resumed = api("/playback", "POST", {"source_id": source["id"], "mode": "direct"})
            assert resumed["position"] == 1.25
            api("/playback/" + resumed["id"], "DELETE")
            print("Docker smoke passed: setup, authorization, scan, real bytes, conversion, persistence and health", flush=True)
        finally:
            subprocess.run([*compose, "logs", "--tail=80"], cwd=root, env=env, timeout=30, check=False)
            subprocess.run([*compose, "down", "--volumes", "--remove-orphans"], cwd=root, env=env, timeout=60, check=False)


if __name__ == "__main__":
    main()
