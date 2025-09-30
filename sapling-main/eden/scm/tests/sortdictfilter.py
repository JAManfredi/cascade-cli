from __future__ import absolute_import

import ast
import pprint
import sys


"""Sort output that looks like a Python dictionary.

Only works for trivial cases.
"""


def main():
    for line in sys.stdin:
        buf = ""  # buffered string representation of a dict
        level = 0  # number of unmatched '{' so far
        for ch in line:
            if ch == "{":
                level += 1
            if level > 0:
                buf += ch
            if ch == "}":
                level -= 1
            if level == 0:
                if buf:
                    try:
                        obj = ast.literal_eval(buf)
                        buf = pprint.pformat(obj, width=sys.maxsize)
                    except Exception:
                        pass
                    sys.stdout.buffer.write(buf.encode())
                    buf = ""
                else:
                    sys.stdout.buffer.write(ch.encode())
        if level > 0 and buf:
            sys.stdout.buffer.write(buf.encode())
    sys.stdout.flush()


if __name__ == "__main__":
    main()
