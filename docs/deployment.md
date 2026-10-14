# Deployment

- Host: the owner's Mac mini (`venus`, macOS arm64, user `zachary`).
- ssh reaches the host through the tunnel's `ssh.zach.ro` route (`ssh://localhost:22`); the client needs `cloudflared` and an `~/.ssh/config` entry with `ProxyCommand cloudflared access ssh --hostname %h`.
- The build runs on the host, from its clone of this repository on `master`.
- A Cloudflare Tunnel (`cloudflared` on the host) terminates TLS and forwards `bacter.zach.ro` to `http://localhost:12001`; the server binds `127.0.0.1` and reads `CF-Connecting-IP` only from the local `cloudflared` (`BACTER_TRUSTED_PROXY_ADDRESSES=127.0.0.1,::1`).
- The server runs as the LaunchAgent `ro.zach.bacter.server` (`~/Library/LaunchAgents/ro.zach.bacter.server.plist`, from `scripts/deploy/ro.zach.bacter.server.template.plist`), running `./scripts/run/run-release-server.sh` from the repository root, restarted by launchd when it exits.
- Log: `~/Library/Logs/bacter/server.log`.
- A LaunchAgent runs only while its user is logged in: the host needs automatic login for `zachary`, or the agent moved to a LaunchDaemon.
- After a reboot with automatic login, launchd loads the agent from `~/Library/LaunchAgents` into the GUI login session itself (`RunAtLoad`); from an ssh session without a GUI login, `deploy.sh` loads it into the user domain `user/<uid>`, which admits it because its `LimitLoadToSessionType` lists `Background` as well as `Aqua`.

## Scripts

- `scripts/deploy/deploy.sh` (on the host): refuses unless on `master` with a clean working tree; `git pull --ff-only`, then re-runs the pulled copy of itself; creates `./.env` from `scripts/deploy/production.template.env` when absent; refuses while a production setting is unset or holds a `<...>` placeholder; `./setup.sh`, which installs the missing required programs; puts the Homebrew and cargo program directories on `PATH`; `./scripts/build/build-release.sh`; installs the LaunchAgent property list (checked with `plutil -lint`); in the launchd domain `gui/<uid>` when `launchctl print gui/<uid>` succeeds (a GUI login session exists), otherwise `user/<uid>`, and prints the domain used: `launchctl bootout` of the agent in each of `gui/<uid>` and `user/<uid>` where it is loaded, then `launchctl bootstrap` into the chosen domain, `launchctl kickstart -k`; fails unless `http://127.0.0.1:<BACTER_PORT>/health` answers 200 within 30 s.
- `scripts/deploy/deploy-remote.sh [--bootstrap] [<ssh host>]` (on the owner's other computer): one ssh call running `deploy.sh` on the host in a login shell; host default `zachary@ssh.zach.ro`; repository path `BACTER_REMOTE_REPOSITORY`, default `~/bacter-rs`; `--bootstrap` clones `https://github.com/zacharysiegel/bacter-rs.git` there first when the path does not exist.
- `scripts/deploy/server-status.sh` (on the host): `launchctl print` of the agent in the domain `deploy.sh` would use, the last 50 log lines, the health check; exits non-zero unless healthy.
- `scripts/deploy/deploy-shared.sh`: the label, paths and functions the scripts above share; sourced, not run.

## First deploy

1. Install Homebrew on the host (https://brew.sh); `deploy.sh` runs `./setup.sh`, which installs the other [system dependencies](./system-dependencies.md).
2. Confirm automatic login for `zachary` on the host.
3. On the host, stop the previous game's service, then confirm that nothing listens on port 12001 (`BACTER_PORT`); this prints nothing:
   ```sh
   lsof -nP -iTCP:12001 -sTCP:LISTEN
   ```
4. From the owner's other computer, clone and deploy; `./.env` is created from `scripts/deploy/production.template.env`, whose `BACTER_ALLOWED_ORIGINS` is `https://bacter.zach.ro`:
   ```sh
   ./scripts/deploy/deploy-remote.sh --bootstrap
   ```

## Routine deploy

- Push to `master`, then:
  ```sh
  ./scripts/deploy/deploy-remote.sh
  ```
- Status, on the host:
  ```sh
  ./scripts/deploy/server-status.sh
  ```
