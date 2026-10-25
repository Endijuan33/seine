# Privacy policy

Last updated: October 25, 2026.

This policy describes hardware and performance telemetry in the Seine miner.
Telemetry helps the project understand which hardware is in use, identify
performance differences and prioritize optimization work.

## Collection and controls

Hardware telemetry is enabled by default in releases configured with a telemetry
collector, including unattended mining. It runs in the background without a
startup prompt. Builds with no collector configured do not send telemetry.
Benchmark mode does not send reports.

To disable it, run Seine with `--telemetry off`, alongside your normal mining
arguments. This preference is saved in `seine.telemetry.json` in your Seine data
directory and applies to later launches. You can also set `enabled` to `false`
in that file. Running instances check the preference before each upload; a
request already in progress may finish. Use `--telemetry on` to enable it again.
Disabling future collection does not immediately delete earlier reports, which
expire under the retention policy below.

## Information reported

Reports contain the CPU model, vendor and architecture identifiers, supported
CPU kernels, core counts, memory capacity, operating system and version, and
whether the miner runs natively or under WSL. Report-time resource snapshots include
the allowed CPU count, OS-estimated available parallelism, effective memory limit
and available memory, including container limits where supported. They include
the miner version, build target and features, and whether it is a debug build.

For active mining backends, reports include GPU model and memory where available,
the number of enabled workers or lanes, CPU kernel, requested page and affinity
policies, allocator-observed memory backing by page type, available GPU tuning
settings, hash counts and measurement durations. Reports
identify measurement windows in which settings changed or telemetry samples
were missed so those windows can be excluded from performance comparisons.

Seine creates a random installation ID in its data directory to distinguish
installations and avoid counting frequent reports as additional users. This ID
is not derived from a wallet, CPU, GPU or other hardware identifier. Each report
also has a random ID to prevent duplicate storage. The collector records when
reports arrive. Installation IDs make reports pseudonymous; they do not make
them anonymous.

Telemetry does not include wallet addresses or balances, private keys, API
tokens, passwords, worker names, pool addresses, hostnames, MAC addresses,
hardware serial numbers, command lines or raw logs.

## Transmission, access and retention

The first report is attempted after five minutes of a mining run, followed by
hourly reports. Reports use HTTPS, except when explicitly configured for a local
loopback test server. Collection failures do not stop mining. Failed reports are
discarded instead of being queued for later upload.

The receiving server necessarily sees the connection's IP address. IP addresses
are not part of report payloads or stored telemetry records. The supplied
deployment configuration disables request and proxy error logging that could
retain IP addresses. Operators who add a CDN or another proxy must configure
its logging consistently with this policy.

Raw reports are retained for 30 days and removed by an hourly cleanup task.
Analysis uses the retained reports to summarize hardware adoption and performance.
The collector exposes no public endpoint for individual reports. Access to the
database is limited to its operators; aggregate hardware and performance results
may be shared to explain optimization work. Reports are used for miner development,
not advertising or wallet profiling. Backups must follow the same retention limit.

The collector URL is recorded in your telemetry settings. Operators of a custom
collector control its hosting and data handling and should provide their own
policy if they change these practices.

## Scope and questions

This telemetry is separate from the connections needed to mine with a pool or
daemon, wallet operations and the miner's update checks. This policy does not
describe those services' data handling or website hosting logs.

Questions about Seine's telemetry can be directed to the
[project maintainers](https://github.com/zpalmtree/seine).
