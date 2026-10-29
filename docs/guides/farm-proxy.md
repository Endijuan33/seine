# Farm proxy

Run one Seine as a proxy on your local network and point every rig at it. The pool
sees a single connection, while each rig mines its own share of the work.

Use it when you run many rigs and want:

- one place to set the pool, payout address and backup pools;
- one outbound pool connection instead of one per rig.

## Start the proxy

```bash
./seine --proxy-listen 0.0.0.0:3333 \
        --pool-url stratum+tcp://bntpool.com:3333 \
        --address YOUR_ADDRESS --pool-worker farm-1
```

The proxy does not mine, so it needs little CPU or memory and can run on any machine
the rigs can reach. Every pool option works here: TLS (`stratum+ssl://`), backup pools,
and `--pool-proxy` for a SOCKS5 proxy. See [pool connections](configuration.md#pool-connections).

## Point the rigs at it

On each rig, use the proxy's LAN address as the pool:

```bash
./seine --pool-url 192.168.1.10:3333 --address YOUR_ADDRESS --pool-worker rig-07
```

Mining OS flight sheets work the same way: put the proxy's address in the pool URL field.

## What to expect

- **Payouts.** Every share through the proxy pays the proxy's `--address`. A rig that logs
  in with a different address is noted once in the proxy's log.
- **Stats.** The pool shows the farm as one worker, named by the proxy's `--pool-worker`.
  The proxy logs connected rigs, accepted and rejected shares and an estimated hashrate
  every minute, and logs each rig as it connects and disconnects.
- **Dev fee.** Each rig still runs its own dev-fee rounds, which connect directly to
  bntpool.com. Rigs therefore need outbound access to bntpool.com:3333 as well as to the
  proxy.
- **Any Blocknet pool.** On pools that hand out extra work on request, the proxy tops up
  without interrupting rigs. On other pools it reconnects for new work when its share
  runs out, and every rig switches to the new job.
- **Upgrades.** At the Faraday network upgrade, rigs on a Seine release without support
  for it are asked to update and disconnected, the same way the pool treats them.

## Security

Anyone who can reach the proxy's port can mine through it, and their shares pay your
address. Listen on a LAN address or firewall the port; do not expose it to the internet.
