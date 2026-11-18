from pathlib import Path

import mkdocs_gen_files


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "protocols" / "rpc-v1.md"
DESTINATION = "protocols/rpc-v1.md"

if not SOURCE.is_file():
    raise FileNotFoundError(f"Missing canonical RPC documentation: {SOURCE}")

with mkdocs_gen_files.open(DESTINATION, "w") as page:
    page.write(SOURCE.read_text(encoding="utf-8"))
