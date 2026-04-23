#!/usr/bin/env python3

import argparse
import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path


ADDRESS_RE = re.compile(r"^0x[0-9a-fA-F]+$")


def parse_args() -> argparse.Namespace:
  parser = argparse.ArgumentParser(
    description="Summarize samply JSON into function-level self-time hotspots."
  )
  parser.add_argument("--input", type=Path, required=True, help="Path to .samply JSON")
  parser.add_argument("--top", type=int, default=30, help="Number of top functions")
  parser.add_argument("--thread", type=int, help="Thread index to analyze (default: auto)")
  parser.add_argument(
    "--include",
    action="append",
    default=[],
    help="Regex include filter for symbol names (repeatable)",
  )
  parser.add_argument(
    "--exclude",
    action="append",
    default=[],
    help="Regex exclude filter for symbol names (repeatable)",
  )
  parser.add_argument(
    "--binary",
    type=Path,
    help="Mach-O binary for atos symbolization fallback (e.g. target/debug/cr)",
  )
  parser.add_argument(
    "--image-base",
    type=lambda raw: int(raw, 0),
    default=0x100000000,
    help="Image base added to frame address before atos (default: 0x100000000)",
  )
  parser.add_argument(
    "--collapse-hash",
    action="store_true",
    help="Collapse Rust symbol hash suffix (::h...) for easier grouping",
  )
  args = parser.parse_args()
  if args.top <= 0:
    parser.error("--top must be > 0")
  return args


def compile_patterns(patterns: list[str]) -> list[re.Pattern[str]]:
  return [re.compile(pattern) for pattern in patterns]


def choose_thread(data: dict, explicit: int | None) -> tuple[int, dict]:
  threads = data.get("threads")
  if not isinstance(threads, list) or not threads:
    raise ValueError("No threads found in .samply file")

  if explicit is not None:
    if explicit < 0 or explicit >= len(threads):
      raise ValueError(f"--thread out of range: {explicit} (total {len(threads)})")
    return explicit, threads[explicit]

  best_idx = 0
  best_count = -1
  for index, thread in enumerate(threads):
    samples = thread.get("samples", {})
    stacks = samples.get("stack", [])
    count = sum(1 for entry in stacks if entry is not None)
    if count > best_count:
      best_count = count
      best_idx = index
  return best_idx, threads[best_idx]


def lookup_frame_symbol(thread: dict, frame_index: int) -> tuple[str, int | None]:
  frame_table = thread.get("frameTable", {})
  func_table = thread.get("funcTable", {})
  native_symbols = thread.get("nativeSymbols", {})
  strings = thread.get("stringArray", [])

  func_index = frame_table.get("func", [None])[frame_index]
  if func_index is not None:
    name_index = func_table.get("name", [None])[func_index]
    if name_index is not None:
      symbol = strings[name_index]
      if ADDRESS_RE.match(symbol):
        return symbol, int(symbol, 16)
      return symbol, None

  native_index = frame_table.get("nativeSymbol", [None])[frame_index]
  if native_index is not None:
    name_index = native_symbols.get("name", [None])[native_index]
    if name_index is not None:
      symbol = strings[name_index]
      if ADDRESS_RE.match(symbol):
        return symbol, int(symbol, 16)
      return symbol, None

  address = frame_table.get("address", [None])[frame_index]
  if address is not None:
    return f"0x{address:x}", address

  return "<unknown>", None


def atos_symbolize(binary: Path, image_base: int, addresses: list[int]) -> dict[int, str]:
  if not addresses:
    return {}

  absolute_addresses = [hex(image_base + address) for address in addresses]
  cmd = ["atos", "-o", str(binary), *absolute_addresses]
  result = subprocess.run(cmd, capture_output=True, text=True)

  mapping: dict[int, str] = {}
  lines = result.stdout.splitlines()
  for address, line in zip(addresses, lines):
    symbol = line.strip()
    if symbol and not ADDRESS_RE.match(symbol):
      mapping[address] = symbol
  return mapping


def normalize_symbol(raw: str, collapse_hash: bool) -> str:
  text = raw.strip()
  if collapse_hash:
    text = re.sub(r"::h[0-9a-f]{16,}$", "", text)
  return text


def summarize(
  thread: dict,
  include_patterns: list[re.Pattern[str]],
  exclude_patterns: list[re.Pattern[str]],
  collapse_hash: bool,
  atos_map: dict[int, str],
) -> tuple[Counter[str], float]:
  stack_table = thread.get("stackTable", {})
  samples = thread.get("samples", {})

  sample_stacks = samples.get("stack", [])
  weights = samples.get("weight")
  if not isinstance(weights, list) or len(weights) != len(sample_stacks):
    weights = [1.0] * len(sample_stacks)

  counts: Counter[str] = Counter()
  total_weight = 0.0
  for index, stack_index in enumerate(sample_stacks):
    if stack_index is None:
      continue
    frame_index = stack_table.get("frame", [None])[stack_index]
    if frame_index is None:
      continue
    symbol, address = lookup_frame_symbol(thread, frame_index)
    if address is not None and address in atos_map:
      symbol = atos_map[address]
    symbol = normalize_symbol(symbol, collapse_hash)

    if include_patterns and not any(regex.search(symbol) for regex in include_patterns):
      continue
    if exclude_patterns and any(regex.search(symbol) for regex in exclude_patterns):
      continue

    weight = float(weights[index])
    counts[symbol] += weight
    total_weight += weight

  return counts, total_weight


def collect_unresolved_addresses(thread: dict, sample_limit: int) -> list[int]:
  stack_table = thread.get("stackTable", {})
  sample_stacks = thread.get("samples", {}).get("stack", [])
  addresses: Counter[int] = Counter()

  for stack_index in sample_stacks:
    if stack_index is None:
      continue
    frame_index = stack_table.get("frame", [None])[stack_index]
    if frame_index is None:
      continue
    symbol, address = lookup_frame_symbol(thread, frame_index)
    if address is None:
      continue
    if ADDRESS_RE.match(symbol):
      addresses[address] += 1

  return [address for address, _ in addresses.most_common(sample_limit)]


def main() -> int:
  args = parse_args()
  if not args.input.exists():
    print(f"Input file not found: {args.input}", file=sys.stderr)
    return 2

  try:
    data = json.loads(args.input.read_text())
  except json.JSONDecodeError as error:
    print(f"Invalid JSON in {args.input}: {error}", file=sys.stderr)
    return 2

  try:
    thread_index, thread = choose_thread(data, args.thread)
  except ValueError as error:
    print(str(error), file=sys.stderr)
    return 2

  include_patterns = compile_patterns(args.include)
  exclude_patterns = compile_patterns(args.exclude)

  atos_map: dict[int, str] = {}
  if args.binary is not None:
    unresolved = collect_unresolved_addresses(thread, sample_limit=max(200, args.top * 10))
    if unresolved:
      atos_map = atos_symbolize(args.binary, args.image_base, unresolved)

  counts, total_weight = summarize(
    thread,
    include_patterns,
    exclude_patterns,
    args.collapse_hash,
    atos_map,
  )

  print(f"Input .samply: {args.input}")
  print(f"Thread index: {thread_index}")
  print(f"Thread name: {thread.get('name', '<unknown>')}")
  print(f"Samples after filter (weight): {total_weight:.2f}")
  if args.binary is not None:
    print(f"Atos binary: {args.binary}")
    print(f"Atos symbolized addresses: {len(atos_map)}")
  print()

  print(f"Top {args.top} Self-Time Hotspots")
  print("-" * 72)
  if not counts:
    print("(no matched symbols)")
    return 0

  for symbol, weight in counts.most_common(args.top):
    ratio = (weight / total_weight * 100.0) if total_weight else 0.0
    print(f"{weight:10.2f}  {ratio:6.2f}%  {symbol}")

  return 0


if __name__ == "__main__":
  raise SystemExit(main())