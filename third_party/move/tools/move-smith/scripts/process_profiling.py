from pathlib import Path
from collections import Counter
from pprint import pprint

CURR = Path(__file__).parent
MOVE_SMITH = CURR.parent

def main():
    data = (MOVE_SMITH / "move-smith-profile.txt").read_text()

    generation_time = []
    transactional_time = []
    status_count = Counter()

    for line in data.splitlines():
        line = line.strip()
        if not line.startswith("move-smith-profile"):
            continue

        parts = line.split("::")
        if parts[1] == "time":
            if parts[2] == "generation":
                generation_time.append(float(parts[3].replace("ms", "")))
            elif parts[2] == "transactional":
                transactional_time.append(float(parts[3].replace("ms", "")))
            else:
                raise ValueError(f"Unknown time type: {parts[2]}")
        elif parts[1] == "status":
            status_count[parts[2]] += 1
        else:
            raise ValueError(f"Unknown type: {parts[1]}")

    total_num = len(generation_time)
    avg_generation_time = sum(generation_time) / len(generation_time)
    avg_transactional_time = sum(transactional_time) / len(transactional_time)
    print(f"Processed {total_num} profiling data points.")
    print(f"Average generation time: {avg_generation_time:.2f}ms")
    print(f"Average transactional time: {avg_transactional_time:.2f}ms")
    print("Status distribution:")
    for status, count in status_count.items():
        percent = count / total_num * 100
        print(f"\t{status}: {count} ({percent:.2f}%)")

if __name__ == '__main__':
    main()
