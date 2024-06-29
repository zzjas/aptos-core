from typing import List
from pathlib import Path

CURR = Path(__file__).parent


def load_prompt(name: str, replaces: List[str]) -> str:
    template = (CURR / f"{name}.md").read_text()
    for i, r in enumerate(replaces):
        template = template.replace(f"__REPLACE_{i}__", r)

    if "__REPLACE" in template:
        raise ValueError("Not enough replacements provided")

    return template
