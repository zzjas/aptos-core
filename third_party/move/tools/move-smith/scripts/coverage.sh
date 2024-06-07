#!/bin/bash

# Usage: ./scripts/coverage.sh <fuzz_target> [base_dir]
#
# `base_dir` should be a directory containing the `corpus/fuzz_target` directory.
#
# This script runs `cargo cov -- show` to generate the coverage report from a
# fuzzing session in HTML format.
# The script should only be run after the fuzzing session has been completed,
# since it uses `fuzz/corpus/<fuzz_target>` to generate the coverage report.
# The coverage report is generated under the `coverage` directory.
# The script can also be used to cleanup the generated coverage files.

MOVE_SMITH_DIR=$(realpath $(dirname $0)/..)

function generate_coverage() {
    local fuzz_target=$1
    local base_dir=${2:-$MOVE_SMITH_DIR}

    local corpus_dir="$base_dir/fuzz/corpus/$fuzz_target"
    local target_dir="$base_dir/coverage"

    mkdir -p $target_dir
    target_dir=$(realpath $target_dir)

    echo "Generating coverage report for $corpus_dir"
    echo "Output directory: $target_dir"

    echo "Collecting coverage data for $fuzz_target"
    export RUSTFLAGS="$RUSTFLAGS -Zcoverage-options=branch"
    cargo fuzz coverage $fuzz_target $corpus_dir

    fuzz_target_bin=$(find target/*/coverage -name $fuzz_target -type f)
    echo "Found fuzz target binary: $fuzz_target_bin"
    # Generate the coverage report
    cargo cov -- show $fuzz_target_bin \
        --format=html \
        --instr-profile=fuzz/coverage/$fuzz_target/coverage.profdata \
        --show-directory-coverage \
        --output-dir=$target_dir \
        -Xdemangler=rustfilt \
        --show-branches=count \
        --ignore-filename-regex='rustc/.*/library|\.cargo'
    echo "Generated coverage report in $target_dir/index.html"
}

curr=$(pwd)
if [ $curr != $MOVE_SMITH_DIR ]; then
    echo "Please run the script from the move-smith directory"
    exit 1
fi

generate_coverage $@