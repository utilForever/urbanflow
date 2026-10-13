"""Run the existing playback assertions with Python 3 and headless Chrome."""

from functools import partial
from html.parser import HTMLParser
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import os
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory, TemporaryFile
from threading import Thread


class Report(HTMLParser):
    def __init__(self):
        super().__init__()
        self.field = None
        self.title = ""
        self.results = ""

    def handle_starttag(self, tag, attrs):
        if tag == "title":
            self.field = "title"
        elif tag == "pre" and dict(attrs).get("id") == "results":
            self.field = "results"

    def handle_endtag(self, tag):
        if tag in ("title", "pre"):
            self.field = None

    def handle_data(self, data):
        if self.field:
            setattr(self, self.field, getattr(self, self.field) + data)


def main():
    root = Path(__file__).resolve().parent.parent
    handler = partial(SimpleHTTPRequestHandler, directory=str(root))
    with ThreadingHTTPServer(("127.0.0.1", 0), handler) as server:
        worker = Thread(target=server.serve_forever, daemon=True)
        worker.start()
        try:
            # Chrome's updater can keep inherited pipes open after Chrome exits.
            with (
                TemporaryDirectory(prefix="rail-playback-") as profile,
                TemporaryFile(mode="w+", encoding="utf-8") as output,
                TemporaryFile(mode="w+", encoding="utf-8") as errors,
            ):
                url = (
                    f"http://127.0.0.1:{server.server_port}"
                    "/tests/rail_viewer_playback.html"
                )
                browser = subprocess.run(
                    [
                        os.environ.get("CHROME_BIN", "google-chrome"),
                        "--headless",
                        f"--user-data-dir={profile}",
                        "--no-first-run",
                        "--no-default-browser-check",
                        "--dump-dom",
                        "--timeout=10000",
                        "--virtual-time-budget=10000",
                        url,
                    ],
                    stdout=output,
                    stderr=errors,
                    timeout=30,
                )
                output.seek(0)
                report = Report()
                report.feed(output.read())
                if browser.returncode:
                    errors.seek(0)
                    print(errors.read(), file=sys.stderr)
        finally:
            server.shutdown()
            worker.join()

    print(report.results or "No browser test results received.")
    if browser.returncode or report.title != "PASS: Rail playback":
        print(
            "Playback assertions failed or did not finish within the browser budget.",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except subprocess.TimeoutExpired:
        sys.exit("Headless Chrome timed out after 30 seconds.")
    except OSError as error:
        sys.exit(f"Could not run playback tests (set CHROME_BIN if needed): {error}")
