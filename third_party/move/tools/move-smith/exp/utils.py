from typing import List
from pathlib import Path
import logging as log
import subprocess
import tempfile
from llm import LLM
from prompts import load_prompt
from rich.progress import track


MOVE_TOML = """[package]
name = "test"
version = "0.0.0"
"""


def create_move_packge(output_dir: Path, code: List[str]):
    sources = output_dir / "sources"
    sources.mkdir(exist_ok=True, parents=True)

    (output_dir / "Move.toml").write_text(MOVE_TOML)
    for i, c in enumerate(code):
        (sources / f"Test_{i}.move").write_text(c)

    log.info(f"Generated Move package at {output_dir}")


def run_transactional_with_code(move_code: str):
    f = tempfile.NamedTemporaryFile(suffix=".move")
    f.write(move_code.encode())
    return run_transactional(Path(f.name))


def run_transactional(move_file: Path):
    if not move_file.exists():
        raise ValueError(f"File {move_file} does not exist")

    result = subprocess.run(
        ["run_transactional", move_file.as_posix()],
        capture_output=True,
        timeout=10,
    )

    return result


def fix_code(llm: LLM, wrong_code: str, errmsg: str):
    prompt = load_prompt("fix", [wrong_code, errmsg])
    return llm.query_json(prompt, format='{"move_code": "FILL_IN_CODE"}')[
        "move_code"
    ]


def check_compile(llm: LLM, check_dir: Path, fix=False):
    log.info(f"Checking all packages in {check_dir}")

    all_packges = [m.parent for m in check_dir.rglob("Move.toml")]
    total = 0
    ok = 0
    for package in track(all_packges, description="Compiling..."):
        total += 1
        log.info(f"Checking package {package}")
        for i in range(1):
            result = subprocess.run(
                ["aptos", "move", "compile"],
                cwd=package,
                capture_output=True,
                timeout=10,
            )
            if result.returncode == 0:
                ok += 1
                break
            log.error(f"Compilation failed for {package}: {result.returncode}")
            if not fix:
                break
            log.error(result.stdout.decode())
            log.error(result.stderr.decode())
            log.info(f"Trying to fix: iteration {i + 1}")

            move_file = list(package.rglob("*.move"))[0]
            fixed = fix_code(
                llm,
                move_file.read_text(),
                result.stderr.decode(),
            )
            move_file.write_text(fixed)

    log.info(f"Checked {total} packages, {ok} compiled successfully")


def check_transactional(check_dir: Path):
    log.info(f"Running all Move files in {check_dir} as transactional tests")

    all_moves = [m for m in check_dir.rglob("sources/*.move")]
    total = 0
    ok = 0
    for move_file in track(
        all_moves, description="Running transactional tests..."
    ):
        total += 1
        result = run_transactional(move_file)
        if result.returncode == 0:
            ok += 1
        else:
            log.error("Transaction test failed:")
            print(move_file)
    log.info(f"Checked {total} files, {ok} runs successfully")
