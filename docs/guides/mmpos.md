# mmpOS

Seine runs on mmpOS as a custom miner. Add it to a miner profile with one
download URL. Your rigs then show seine's hashrate per GPU and its accepted and
rejected shares on the mmpOS dashboard.

Each release has an mmpOS package named `seine-mmpos-<version>.tar.gz`. It holds
a seine build that runs on older system libraries, the CUDA NVRTC library seine
needs to compile its NVIDIA kernels, and `jq` for the stats script. You don't
need to install anything else on the rig.

## Set up a miner profile

1. **Add a wallet** with your Blocknet payout address and the coin `BNT`. Seine
   also accepts a Blocknet ID shortname such as `@name`. Use the `@` form,
   because mmpOS runs the command line through a shell, which treats `$`
   specially.
2. **Add a pool** for `BNT`: server `bntpool.com`, port `3333`, SSL off. Keep the
   default username, `%wallet_address%.%rig_name%%miner_id%`. Blocknet pools
   don't use passwords, so the password can stay `x`.
3. **Create a miner profile** for `BNT`. Choose **Custom miner** as the miner and
   paste this into **Custom miner download url**:

   ```text
   https://github.com/zpalmtree/seine/releases/download/vX.Y.Z/seine-mmpos-X.Y.Z.tar.gz
   ```

   Replace `X.Y.Z` with the newest version on the
   [releases page](https://github.com/zpalmtree/seine/releases/latest), or copy
   the link to the `seine-mmpos-…tar.gz` file there. Select the wallet and the
   pool from step 2.
4. **Check the arguments** under the profile's advanced settings. Keep the
   arguments mmpOS fills in for custom miners:

   ```text
   ./mmp-launch.sh --coin %coin% --pool %pool_server%:%pool_port% --user %user% --password %password% --api-port %api_port%
   ```

   Add seine flags at the end of the line (see below).
5. **Assign the profile** to your rigs.

To mine on a pool's TLS port, turn on SSL for that pool in mmpOS and add
`%pool_protocol%` after `--coin %coin%` in the arguments. Seine then connects
with `stratum+ssl://`. If the profile has no pool, seine mines on
`stratum+tcp://bntpool.com:3333`.

On the first start, seine tunes itself to each GPU for a few minutes before
hashrate appears. It saves the result, so later starts begin hashing within
seconds.

## What it mines on

- **GPU rigs mine on the GPUs only.** Every hash takes 2 GiB of memory: VRAM on a
  GPU, system RAM on the CPU. Many GPU rigs have 4–8 GB of RAM, so CPU mining on
  top of the GPUs could run the rig out of memory. That's why the package leaves
  the CPU off.
- **Rigs without a GPU mine on the CPU.** Seine sizes the thread count to the free
  RAM and keeps some RAM back for the system.
- **NVIDIA** needs a driver with CUDA 12 support. RTX 50 series cards need driver
  570 or newer.
- **AMD Radeon RX 6000, 7000 and 9000** cards (RDNA) need the ROCm HIP runtime
  (`libamdhip64`). If the rig doesn't have it, seine says so at startup and
  leaves those cards idle. Older AMD cards (RX 400/500, Vega) are not supported.
- Seine needs an mmpOS image built on Ubuntu 20.04 or newer (glibc 2.31). On an
  older image it logs `seine needs 2.31 or newer` and doesn't start.

### Add CPU mining to a GPU rig

Add the backends you want to the end of the arguments. Plan on one thread per
2 GiB of RAM you can spare, and leave at least 2 GB for mmpOS.

```text
--backend nvidia,cpu --threads 2
```

Use `amd,cpu` on AMD rigs, or `nvidia,amd,cpu` on mixed rigs. If you leave out
`--threads`, seine sizes and tunes the CPU threads itself to the free RAM.

## More seine flags

Anything after the mmpOS placeholders goes to seine as is, and your value wins
over the package defaults. Some examples:

| Add to the arguments | Effect |
|---|---|
| `--pool-url stratum+tcp://bntpool.com:3333,stratum+ssl://backup.example.com:3443` | Backup pools: seine tries them in order and returns to the first when it recovers |
| `--backend nvidia --nvidia-devices 0,1,3` | Mine on some NVIDIA GPUs only |
| `--nvidia-max-lanes 4` | Use less VRAM on each NVIDIA GPU |
| `--cpu-page-mode regular` | CPU mining without huge pages |
| `--telemetry off` | Don't send hardware reports |

See the [flag reference](../MINER_FLAGS.md) for every option. Quote values that
contain spaces.

The package always adds these flags, unless you set them yourself:

| Flag | Why |
|---|---|
| `--backend nvidia` / `amd` / `cpu` | Picked from the GPUs mmpOS detects (see above) |
| `--ui plain` | Plain log lines for the mmpOS miner log |
| `--no-auto-update` | mmpOS installs new versions through the package |
| `--data-dir /opt/mmp/miners/seine-data` | Tuning results survive package upgrades |
| `--api-server --api-bind 127.0.0.1:<Api port>` | Local stats for the mmpOS dashboard |

The control API listens on the profile's **Api port** (4444 unless you change it),
or on 9977 if the port is empty or 0. Don't change `--api-bind`, `--api-cookie` or `--api-token` by hand:
the dashboard stats read seine's local control API at that address.

## Huge pages

GPU mining doesn't use huge pages. For CPU mining on Linux, huge pages are worth a
few percent (see [Linux huge pages](linux-hugepages.md)). The package doesn't
change system settings, so reserve the pages yourself if you want them. Reserved
huge pages are unavailable to the rest of the system, so only reserve what the
CPU threads use.

## Upgrading

Seine doesn't update itself on mmpOS. When a newer release is out, seine logs a
notice. To upgrade, change the version in the profile's download URL to the new
release. mmpOS downloads each URL once, so the new URL fetches the new package.
Your tuning results and settings stay in `/opt/mmp/miners/seine-data`.

## Troubleshooting

- The miner log on the dashboard shows seine's output, starting with the full
  seine command line.
- **"no wallet"**: the profile's pool has no wallet address. Pick the pool and
  wallet in the miner profile.
- **Hashrate shows 0 right after starting**: seine is tuning the GPUs on the first
  start. Wait a few minutes.
- **NVIDIA GPUs aren't mining**: check the log for CUDA or driver errors. Seine
  keeps mining on the other GPUs and logs a `running degraded` warning.
- Shares and payouts for bntpool.com are on the pool's dashboard. Search for your
  address there.
