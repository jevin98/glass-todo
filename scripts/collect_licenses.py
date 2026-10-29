"""Collect declared dependency license texts from the locked Cargo package sources."""
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1"], cwd=ROOT, text=True, encoding="utf-8"))
    output = ROOT / "third-party"
    texts = output / "licenses"
    texts.mkdir(parents=True, exist_ok=True)
    rows = []
    missing = []
    for package in sorted(metadata["packages"], key=lambda item: (item["name"], item["version"])):
        if package["id"] in metadata["workspace_members"]:
            continue
        source = Path(package["manifest_path"]).parent.resolve()
        candidates = set()
        for pattern in ("LICENSE*", "COPYING*", "COPYRIGHT*", "NOTICE*"):
            candidates.update(path for path in source.glob(pattern) if path.is_file())
        if package.get("license_file"):
            candidates.add(source / package["license_file"])
        if (source / "licenses").is_dir():
            candidates.update(path for path in (source / "licenses").rglob("*") if path.is_file())
        notices = []
        for file in sorted(candidates):
            if not file.resolve().is_relative_to(source) or not file.is_file():
                continue
            content = file.read_bytes().decode("utf-8-sig").replace("\r\n", "\n")
            digest = hashlib.sha256(content.encode()).hexdigest()[:32]
            (texts / f"{digest}.txt").write_text(content, encoding="utf-8")
            notices.append(f"[{file.name}](licenses/{digest}.txt)")
        if not notices:
            missing.append(package["name"])
        name, version = package["name"], package["version"]
        license_ = package.get("license") or "See upstream package"
        rows.append(f"| [{name}](https://crates.io/crates/{name}/{version}) | {version} | {license_} | {', '.join(notices) or 'See upstream package'} |")
    sysroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip())
    rust_files = []
    for name in ("COPYRIGHT", "LICENSE-APACHE", "LICENSE-MIT"):
        for location in (sysroot / "share/doc/rust" / name, sysroot / name):
            if location.is_file():
                (output / f"RUST-{name}.txt").write_bytes(location.read_bytes())
                rust_files.append(f"[Rust {name}](RUST-{name}.txt)")
                break
    compiler = subprocess.check_output(["rustc", "--version"], text=True).strip()
    header = (
        "# Third-party notices\n\n"
        "GlassTodo's own code is MIT-licensed. Dependencies retain their upstream licenses. "
        "This inventory is generated from Cargo.lock and package license files; it includes resolved platform/build dependencies, "
        "including some not linked for every target. It is an inventory, not a new license grant.\n\n"
        "Regenerate with `python scripts/collect_licenses.py` after changing dependencies/toolchains. "
        "CI regenerates the inventory for its package.\n\n"
        f"Compiler: `{compiler}`. " + ('; '.join(rust_files) if rust_files else 'This minimal toolchain does not include local Rust notice files; see the [upstream Rust license texts](https://github.com/rust-lang/rust#license).') + '\n\n'
        "SQLite is bundled by libsqlite3-sys; its upstream [public-domain statement](https://sqlite.org/copyright.html) "
        "applies in addition to the wrapper crate's notice. Windows APIs, Rich Edit and system fonts are used from the OS, not redistributed here.\n\n"
        "| Package | Locked version | Declared SPDX expression | Included upstream text |\n"
        "| --- | --- | --- | --- |\n"
    )
    (output / "README.md").write_text(header + "\n".join(rows) + "\n", encoding="utf-8")
    print(json.dumps({"dependency_packages": len(rows), "unique_license_texts": len(list(texts.glob('*.txt'))), "packages_without_packaged_notice": missing}))


if __name__ == "__main__":
    main()
