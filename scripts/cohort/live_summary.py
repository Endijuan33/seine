"""One summary line for a pool_fixture.py run.

Users leave miners running for hours, so competitor comparisons use the steady rate:
the median per-minute rate of all eligible shares (user + fee work) once the miner has
settled (from minute 5, or a minute after its first share if later), times one minus
the miner's nominal fee. Lifetime and last-300 s user rates still show startup cost.
"""
import json
import math
import statistics
import sys

# Nominal dev fees; fee work that reaches the fixture is counted as gross work.
FEES = {"seine": 0.02, "seine-base": 0.02, "ntmminer": 0.0, "longhorn": 0.02}

d = json.load(open(sys.argv[1]))
m, secs = sys.argv[2], float(sys.argv[3])
shares = [s for s in d["shares"] if s["eligible"] and s["elapsed"] < secs]
user = [s["elapsed"] for s in shares if s["user_work"]]
fee = len(shares) - len(user)
last = sum(1 for t in user if t >= secs - 300) / 300
first = d["launch_to_first_share_seconds"] or secs
start = math.ceil(max(300, first + 60) / 60) * 60
minutes = [sum(1 for s in shares if a <= s["elapsed"] < a + 60) / 60 for a in range(start, int(secs) - 59, 60)]
steady = statistics.median(minutes) if minutes else 0.0
print(f"{m} steady_fee_adj={steady * (1 - FEES.get(m, 0.0)):.3f} steady_gross={steady:.3f} "
      f"lifetime_user={len(user)/secs:.3f} last300_user={last:.3f} fee_shares={fee} "
      f"first_share={d['launch_to_first_share_seconds']} end={d['termination_reason']}")
