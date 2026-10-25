---
title: How to mine Blocknet in under a minute
description: A step-by-step guide to mining Blocknet (BNT) on Windows, macOS or Linux with seine, the fastest free Blocknet miner, plus tips to get more hashrate.
date: 2026-10-25
---

Blocknet uses a memory-hard proof of work (Argon2id with 2 GiB per hash), so ordinary
computers can mine it: desktops, laptops, servers and NVIDIA GPUs. This guide gets you
mining with **seine**, a free, open-source miner that tunes itself to your hardware.

## What you need

- A Blocknet address to be paid to, or a registered Blocknet ID shortname (`@name`).
- About **2 GB of RAM per CPU thread**. Seine sizes itself to your machine, so you don't
  need to work this out.
- Windows, macOS or Linux. NVIDIA GPUs are used automatically when a recent driver is
  installed.

## 1. Download seine

Grab the archive for your platform from the [download section](/#download). There is
nothing to install: unzip it and you're ready. On Windows the archive already includes the
NVIDIA runtime files seine needs.

## 2. Run it

On Windows, double-click `seine.exe`. On macOS or Linux, open a terminal in the folder
and run:

```
./seine
```

## 3. Enter your address

Seine asks for your payout address (or `@shortname`) and a pool. Accept the default,
[bntpool.com](https://bntpool.com), and you're mining. On bntpool.com seine's dev fee is
1%; on other pools it's 2.5%.

From here seine keeps tuning while it mines: it compares thread counts and CPU kernels
using real work, and keeps whatever is fastest on your machine.

## Getting more hashrate

- **Mine on CPU and GPU together.** Seine does this by default; both hashrates add up.
- **Linux and WSL: reserve huge pages.** Backing each thread's memory with huge pages adds
  about 4–8% on CPU. See the [huge pages guide](https://github.com/zpalmtree/seine/blob/master/docs/guides/linux-hugepages.md).
- **Big servers.** Seine spreads threads across NUMA nodes and cache groups automatically.
  See the [large servers guide](https://github.com/zpalmtree/seine/blob/master/docs/guides/large-servers.md).
- **Apple Silicon.** CPU-only mining is the fastest choice on M-series Macs; a 48 GB M4 Max
  reaches about 29–31 H/s.

## How fast is it?

Measured peak results include **44.7 H/s on an AMD EPYC Zen 5 server** and **30.9 H/s on an
Apple M4 Max**. Results for every CPU we've tested are on the
[performance page](https://github.com/zpalmtree/seine/blob/master/docs/PERFORMANCE.md).
