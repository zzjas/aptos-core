import logging
from rich.logging import RichHandler
from llm import LLM
from prompts import load_prompt
from pathlib import Path
from utils import (
    create_move_packge,
    run_transactional_with_code,
    check_compile,
    check_transactional,
)
from rich.progress import track

CURR = Path(__file__).parent
OUTPUT_DIR = CURR / "output"

FORMAT = "%(message)s"
logging.basicConfig(
    level="INFO", format=FORMAT, datefmt="[%X]", handlers=[RichHandler()]
)
log = logging.getLogger("rich")

ALL_FEATURES = [
    "primitive types",
    "binary operators",
    "comparison",
    "casting",
    "vector",
    "byte strings",
    "hex strings",
    "vector operations",
    "vector slice",
    "vector rotate",
    "vector remove",
    "references",
    "mutable references",
    "freeze",
    "subtyping",
    "ownership",
    "tuple",
    "unit",
    "let binding",
    "type annotation",
    "optional type annotation",
    "multiple declaration",
    "tuple destruction",
    "struct destruction",
    "destructuring references",
    "ignoring values",
    "assignments",
    "expression blocks",
    "shadowing",
    "move and copy",
    "equality",
    "abort",
    "conditionals",
    "while loop",
    "for loop",
    "break",
    "continue",
    "the loop expression",
    "function visibility",
    "entry function",
    "function type parameters",
    "function acquires",
    "native functions",
    "inline function",
    "struct pattern matching",
    "borrowing structs and fields",
    "field read and write",
    "privileged struct operations",
    "constant",
    "generic functions",
    "generic structs",
    "type argument",
    "type inference",
    "phantom type parameters",
    "generic type instantiation",
    "type parameter constraints",
    "the copy ability",
    "the drop ability",
    "the store ability",
    "the key ability",
    "uses other module",
    "aliases",
    "friend modules",
    "global storage",
    "the move_to operation",
    "the move_from operation",
    "the borrow_global operation",
    "the borrow_global_mut operation",
    "the exists operation",
]

ALL_FEATURES = ALL_FEATURES[:5]


def gen_test_for_one_feature(llm: LLM, output_dir: Path, feature: str):
    log.info(f"Genearting test for feature: {feature}")
    prompt = load_prompt("transactional", ["inline function"])

    code = llm.query_json(prompt, format='{"move_code": "FILL_IN_CODE"}')[
        "move_code"
    ]
    log.info(
        f"Got Move code from LLM for {feature}. Length {len(code.splitlines())}"
    )
    create_move_packge(output_dir, [code])


def gen_all_features(llm: LLM):
    output_dir = OUTPUT_DIR / "features"
    log.info(f"Generating tests for {len(ALL_FEATURES)} features")
    for f in track(ALL_FEATURES, description="Generating tests..."):
        for i in range(3):
            name = f.replace(" ", "_") + f"_{i}"
            gen_test_for_one_feature(llm, output_dir / name, f)
    log.info("Done generating tests for all features")


def main():
    llm = LLM(
        sysmsg="You are an expert in the programming language Move from Aptos. "
        "You will write extremely high-quality Move code without syntax errors."
        " You also understand the Move compiler and Move VM by heart."
        " You are awesome!"
    )
    gen_all_features(llm)
    check_compile(llm, OUTPUT_DIR)
    check_transactional(OUTPUT_DIR)


if __name__ == "__main__":
    main()
