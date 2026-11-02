"""A fake pub server for differential tests of `pub publish`.

usage: python3 -I fakepub.py <port> <mode> <data dir>

Serves the version listings and archives of the packages under
<data dir>/packages/<name>/<version>.{tar.gz,json} (the .json file is the
pubspec as JSON) and implements the publishing protocol: GET
/api/packages/versions/new, a multipart POST to /upload and GET /finalize.
<mode> selects failures: ok, auth401, auth403, new500, newbad, newnourl,
gcs, noloc, finerr, finbad. Every request is appended to
<data dir>/requests-<port>.log (method, path, the headers that matter and,
for uploads, the form fields and the SHA-256 of the archive); uploaded
archives are stored as <data dir>/uploads/<port>-<n>.tar.gz.
"""
import hashlib
import http.server
import json
import os
import sys

PORT = int(sys.argv[1])
MODE = sys.argv[2]
DATA = sys.argv[3]
UPLOADS = [0]


def log(line):
    with open(os.path.join(DATA, "requests-all.log"), "a") as f:
        f.write("%d %s\n" % (PORT, line))


def listing(host, name):
    directory = os.path.join(DATA, "packages", name)
    if not os.path.isdir(directory):
        return None
    versions = []
    for entry in sorted(os.listdir(directory)):
        if not entry.endswith(".json"):
            continue
        version = entry[: -len(".json")]
        with open(os.path.join(directory, entry)) as f:
            pubspec = json.load(f)
        archive = os.path.join(directory, version + ".tar.gz")
        with open(archive, "rb") as f:
            sha = hashlib.sha256(f.read()).hexdigest()
        row = {
            "version": version,
            "pubspec": pubspec,
            "archive_url": "http://%s/packages/%s/versions/%s.tar.gz" % (host, name, version),
            "archive_sha256": sha,
        }
        if pubspec.get("_retracted"):
            row["retracted"] = True
            del pubspec["_retracted"]
        versions.append(row)
    return {"name": name, "latest": versions[-1], "versions": versions}


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):
        pass

    def record(self, extra=""):
        auth = self.headers.get("authorization")
        content_type = self.headers.get("content-type") or ""
        if content_type.startswith("multipart/form-data"):
            content_type = "multipart/form-data"
        log(
            "%s %s accept=%s auth=%s type=%s agent=%s%s"
            % (
                self.command,
                self.path,
                self.headers.get("accept"),
                auth,
                content_type,
                self.headers.get("user-agent"),
                extra,
            )
        )

    def send(self, status, body=b"", headers=None, content_type="application/json"):
        if isinstance(body, (dict, list)):
            body = json.dumps(body).encode()
        elif isinstance(body, str):
            body = body.encode()
        self.send_response(status)
        self.send_header("content-type", content_type)
        self.send_header("content-length", str(len(body)))
        for key, value in (headers or {}).items():
            self.send_header(key, value)
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        self.record()
        host = self.headers.get("host")
        if self.path == "/api/packages/versions/new":
            if MODE == "auth401":
                return self.send(401, {"error": {"message": "no"}}, {"www-authenticate": 'Bearer realm="pub", message="Your token was revoked."'})
            if MODE == "auth403":
                return self.send(403, {"error": {"message": "no"}}, {"www-authenticate": 'Bearer realm="pub", message="Not an uploader of this package."'})
            if MODE == "new500":
                return self.send(500, "boom", content_type="text/plain")
            if MODE == "newbad":
                return self.send(200, "this is not json", content_type="text/plain")
            if MODE == "newnourl":
                return self.send(200, {"fields": {}})
            return self.send(200, {"url": "http://%s/upload" % host, "fields": {"key": "value", "acl": "private"}})
        if self.path == "/finalize":
            if MODE == "finerr":
                return self.send(400, {"error": {"message": "Version 1.0.0 of package simple already exists."}})
            if MODE == "finbad":
                return self.send(200, {"nope": 1})
            return self.send(200, {"success": {"message": "Successfully uploaded https://pub.dev/packages/x version 1.0.0."}})
        if self.path.startswith("/api/packages/"):
            name = self.path[len("/api/packages/"):]
            body = listing(host, name)
            if body is None:
                return self.send(404, {"error": {"code": "NotFound", "message": "Package not found."}})
            return self.send(200, body, content_type="application/vnd.pub.v2+json")
        if self.path.startswith("/packages/") and self.path.endswith(".tar.gz"):
            parts = self.path.split("/")
            name, version = parts[2], parts[4][: -len(".tar.gz")]
            path = os.path.join(DATA, "packages", name, version + ".tar.gz")
            if os.path.exists(path):
                with open(path, "rb") as f:
                    return self.send(200, f.read(), content_type="application/octet-stream")
        return self.send(404, {"error": {"message": "not found"}})

    def do_POST(self):
        length = int(self.headers.get("content-length") or 0)
        body = self.rfile.read(length)
        content_type = self.headers.get("content-type") or ""
        boundary = content_type.split("boundary=")[-1].encode()
        fields = []
        archive = b""
        for part in body.split(b"--" + boundary):
            if b"\r\n\r\n" not in part:
                continue
            head, data = part.split(b"\r\n\r\n", 1)
            if data.endswith(b"\r\n"):
                data = data[:-2]
            head = head.decode(errors="replace").lower()
            if 'name="file"' in head:
                archive = data
                fields.append("file(%s)" % ("filename=\"package.tar.gz\"" in head))
            elif "name=" in head:
                name = head.split('name="')[1].split('"')[0]
                fields.append("%s=%s" % (name, data.decode(errors="replace")))
        self.record(" fields=%s sha256=%s" % (",".join(fields), hashlib.sha256(archive).hexdigest()))
        UPLOADS[0] += 1
        os.makedirs(os.path.join(DATA, "uploads"), exist_ok=True)
        with open(os.path.join(DATA, "uploads", "%d-%d.tar.gz" % (PORT, UPLOADS[0])), "wb") as f:
            f.write(archive)
        host = self.headers.get("host")
        if MODE == "gcs":
            return self.send(400, "<?xml version='1.0' encoding='UTF-8'?><Error><Code>EntityTooLarge</Code><Message>Your proposed upload is larger than the maximum object size specified in your Policy Document.</Message><Details>Content-length exceeds upper bound on range</Details></Error>", content_type="application/xml")
        if MODE == "noloc":
            return self.send(204)
        return self.send(302, b"", {"location": "http://%s/finalize" % host})


http.server.ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
