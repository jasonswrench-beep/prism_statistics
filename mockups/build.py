"""Build the mockup from src/index.html with the logo embedded.

Writes:
  index.html          full standalone page for opening locally
  dist/artifact.html  same page without the doctype/html/head/body wrapper, for publishing as an Artifact
"""
import base64, pathlib, re
root = pathlib.Path(__file__).resolve().parent
logo = (root.parent / "PrismStatistics.svg").read_bytes()
uri = "data:image/svg+xml;base64," + base64.b64encode(logo).decode()
html = (root / "src" / "index.html").read_text(encoding="utf-8").replace("{{LOGO}}", uri)
(root / "index.html").write_text(html, encoding="utf-8")
frag = re.sub(r"<!doctype html>\s*|</?html[^>]*>\s*|</?head>\s*|</?body>\s*", "", html, flags=re.I)
frag = re.sub(r'<meta charset="utf-8">\s*|<meta name="viewport"[^>]*>\s*', "", frag)
(root / "dist").mkdir(exist_ok=True)
(root / "dist" / "artifact.html").write_text(frag, encoding="utf-8")
print("wrote index.html and dist/artifact.html")
