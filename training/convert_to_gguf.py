"""Conversion harness for trained Legion adapters with optional real and fixture paths."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


def _build_manifest(
    model_dir: Path,
    output: Path,
    quantization: str,
    llama_cpp_convert_script: Path | None,
    quantize_command: Path | None,
    metadata_output: Path | None,
    mode: str,
    commands: list[list[str]] | None = None,
) -> dict[str, Any]:
    manifest = {
        "mode": mode,
        "model_dir": str(model_dir),
        "output": str(output),
        "quantization": quantization,
        "llama_cpp_convert_script": str(llama_cpp_convert_script) if llama_cpp_convert_script else None,
        "quantize_command": str(quantize_command) if quantize_command else None,
        "metadata_output": str(metadata_output) if metadata_output else None,
        "commands": commands or [],
    }
    return manifest


def _write_manifest(path: Path, manifest: dict[str, Any]) -> None:
    path.write_text(json.dumps(manifest, indent=2, sort_keys=True), encoding="utf-8")


def _validate_tool(path: Path, label: str) -> None:
    if not path.exists():
        raise SystemExit(f"{label} does not exist: {path}")
    if not path.is_file():
        raise SystemExit(f"{label} is not a file: {path}")


def _run_fixture_smoke(
    model_dir: Path,
    output: Path,
    quantization: str,
    metadata_output: Path | None,
) -> int:
    """Smoke path: verify command construction without invoking real llama.cpp."""
    if not model_dir.exists():
        raise SystemExit(f"model_dir does not exist: {model_dir}")

    # Simulate the conversion command list (no shell interpolation)
    convert_args = [
        "python3",
        "convert_hf_to_gguf.py",
        "--outfile",
        str(output),
        str(model_dir),
    ]
    quantize_args = [
        "llama-quantize",
        str(output),
        str(output.with_suffix(f".{quantization}.gguf")),
        quantization,
    ]

    manifest = _build_manifest(
        model_dir=model_dir,
        output=output,
        quantization=quantization,
        llama_cpp_convert_script=None,
        quantize_command=None,
        metadata_output=metadata_output,
        mode="fixture-smoke",
        commands=[convert_args, quantize_args],
    )
    if metadata_output:
        _write_manifest(metadata_output, manifest)
    print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0


def _run_real(
    model_dir: Path,
    output: Path,
    quantization: str,
    llama_cpp_convert_script: Path | None,
    quantize_command: Path | None,
    metadata_output: Path | None,
) -> int:
    """Real conversion: validate tools and run subprocess with explicit arg lists."""
    if not model_dir.exists():
        raise SystemExit(f"model_dir does not exist: {model_dir}")

    convert_script = llama_cpp_convert_script or Path("convert_hf_to_gguf.py")
    quantize_bin = quantize_command or Path("llama-quantize")

    _validate_tool(convert_script, "llama_cpp_convert_script")
    _validate_tool(quantize_bin, "quantize_command")

    # Build explicit arg lists — no shell interpolation
    convert_args: list[str] = [
        sys.executable,
        str(convert_script),
        "--outfile",
        str(output),
        str(model_dir),
    ]
    quantize_args: list[str] = [
        str(quantize_bin),
        str(output),
        str(output.with_suffix(f".{quantization}.gguf")),
        quantization,
    ]

    commands = [convert_args, quantize_args]
    for cmd in commands:
        print(f"running: {' '.join(cmd)}", file=sys.stderr)
        result = subprocess.run(cmd, capture_output=True, text=True, check=False)
        if result.returncode != 0:
            print(f"command failed: {cmd}", file=sys.stderr)
            print(f"stderr: {result.stderr}", file=sys.stderr)
            return result.returncode

    manifest = _build_manifest(
        model_dir=model_dir,
        output=output,
        quantization=quantization,
        llama_cpp_convert_script=convert_script,
        quantize_command=quantize_bin,
        metadata_output=metadata_output,
        mode="real",
        commands=commands,
    )
    if metadata_output:
        _write_manifest(metadata_output, manifest)
    print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0


def main() -> int:
    """Validate conversion arguments and print an operator plan."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--input", default="training/out/docs-summarizer")
    parser.add_argument("--model-dir", default="")
    parser.add_argument("--output", default="models/docs-summarizer.Q4_K_M.gguf")
    parser.add_argument("--quantization", default="Q4_K_M")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--llama-cpp-convert-script", default="")
    parser.add_argument("--quantize-command", default="")
    parser.add_argument("--metadata-output", default="")
    parser.add_argument("--fixture-smoke", action="store_true")
    args = parser.parse_args()

    model_dir = Path(args.model_dir) if args.model_dir else Path(args.input)
    output = Path(args.output)
    llama_cpp_convert_script = (
        Path(args.llama_cpp_convert_script) if args.llama_cpp_convert_script else None
    )
    quantize_command = Path(args.quantize_command) if args.quantize_command else None
    metadata_output = Path(args.metadata_output) if args.metadata_output else None

    if args.dry_run:
        plan = {
            "dry_run": True,
            "input": str(model_dir),
            "output": str(output),
            "quantization": args.quantization,
            "tool": "llama.cpp convert + quantize",
        }
        print(json.dumps(plan, indent=2, sort_keys=True))
        return 0

    if args.fixture_smoke:
        return _run_fixture_smoke(
            model_dir,
            output,
            args.quantization,
            metadata_output,
        )

    return _run_real(
        model_dir,
        output,
        args.quantization,
        llama_cpp_convert_script,
        quantize_command,
        metadata_output,
    )


if __name__ == "__main__":
    raise SystemExit(main())
