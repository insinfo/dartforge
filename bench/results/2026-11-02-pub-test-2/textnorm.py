"""Normalizes a text reporter's output for comparison.

usage: textnorm.py <file>

`\r` updates of the compact reporter become separate lines; elapsed times
(`MM:SS`) become `TT:TT`; the padding the compact reporter adds up to the
line length is kept (it depends on the message, not on time, except for the
time digits themselves, which have the same width). Stack frames are kept, so
the remaining differences are exactly the frames.
"""
import re
import sys

text = open(sys.argv[1], encoding="utf-8", errors="replace").read()
text = text.replace("\r", "\n")
text = re.sub(r"\b\d\d:\d\d\b", "TT:TT", text)
# The github reporter prints elapsed times as "(1 ms)", "(2.1s)".
text = re.sub(r"\(\d+(\.\d+)?\s?m?s\)", "(T)", text)
# The compact reporter repaints its line every second while a suite loads;
# how many repaints happen depends only on time.
lines = []
for line in text.split("\n"):
    if not lines or lines[-1] != line or line == "":
        lines.append(line)
sys.stdout.write("\n".join(lines))
