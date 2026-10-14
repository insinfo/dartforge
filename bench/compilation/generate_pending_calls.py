"""Generate ordinary Dart with many used exception edges and unused call results."""

import argparse
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--calls", type=int, default=500)
    args = parser.parse_args()
    if args.calls < 1:
        parser.error("--calls must be positive")
    source = """int counter = 0;

@pragma('vm:never-inline')
int step(int value) {
  if (value < 0) throw StateError('negative');
  counter += value;
  return counter;
}

void main() {
"""
    source += "".join(f"  step({value});\n" for value in range(1, args.calls + 1))
    source += """  try {
    step(-1);
  } on StateError {
    print('caught');
  }
  print(counter);
}
"""
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(source)


if __name__ == "__main__":
    main()
