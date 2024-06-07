#!/bin/bash

# This script runs a fuzz target for a given number of hours.
#
# Usage: ./scripts/fuzz.sh <fuzz_target> <total_hour> <max_input_len>
#
# * Keep a log file under move-smith/logs
# * Creates an initial corpus with 8KB inputs
# * Runs the fuzz target for the given number of hours
# * Uses 10 forks for libfuzzer

MOVE_SMITH_DIR=$(realpath $(dirname $0)/..)

function create_log() {
    local log_dir=$1
    mkdir -p $log_dir
    # Count the number of files in the logs directory
    log_count=$(ls logs | wc -l | xargs)
    # Return the next log file name
    echo "$log_dir/fuzz-$log_count.log"
}

# This is needed since libfuzzer doesn't respect the -max_len flag
function create_initial_corpus() {
    local fuzz_target=$1
    local input_len=$2    # in KB

    local corpus_dir=$MOVE_SMITH_DIR/fuzz/corpus/$fuzz_target
    mkdir -p $corpus_dir

    for i in {0..9}; do
        large=$corpus_dir/random_input_large_$i
        mid=$corpus_dir/random_input_mid_$i
        small=$corpus_dir/random_input_small_$i
        dd if=/dev/urandom of=$large ibs=1024 count=$input_len 2>/dev/null
        dd if=/dev/urandom of=$mid ibs=512 count=$input_len 2>/dev/null
        dd if=/dev/urandom of=$small ibs=256 count=$input_len 2>/dev/null
    done
}

function run_fuzz() {
    local fuzz_target=${1:-"transactional"}
    local total_hour=${2:-24}    # Default to 24 hours
    local input_len=${3:-4}      # Default to 4 KB

    # Convert hours to seconds, convert to integer
    local total_seconds=$(echo "$total_hour * 3600" | bc)
    local log_file=$(create_log "$MOVE_SMITH_DIR/logs")
    echo "Writing logs to $log_file"

    create_initial_corpus $fuzz_target $input_len

    echo "Current date time: $(date)" | tee -a $log_file
    echo "Created initial corpus for $fuzz_target, size: $input_len KB" | tee -a $log_file
    echo "Running fuzz target: $fuzz_target for $total_hour hours" | tee -a $log_file

    # Disable ASAN only on Linux
    # Disabling ASAN on macOS fails to build
    local asan_flag=""
    if [[ "$OSTYPE" == "linux-gnu" ]]; then
        asan_flag="-s=none"
    fi
    echo "ASAN flag: $asan_flag" | tee -a $log_file

    cargo fuzz run $asan_flag $fuzz_target -- \
        -max_total_time=$total_seconds \
        -max_len=819200 \
        -keep_seed=1 \
        -fork=10 \
        -timeout=20 \
        -ignore_timeouts=1 \
        -ignore_crashes=1 \
        -print_final_stats=1 2>&1 | tee -a $log_file
}

if [ "$#" -ne 3 ]; then
    echo "Usage: ./scripts/fuzz.sh <fuzz_target> <total_hour> <max_input_len>"
    exit 1
fi

run_fuzz $@