# Feather on a server — instructions for an AI assistant

You are on the person's Linux VPS. The goal is to put **Feather Chat** online so
their phone and computer can use it, without breaking anything already running
on the server.

Read this whole file once, then follow the order below. Prefer running the
provided installer over inventing commands. Never publish port 8777 directly
to the internet.

## What is in this folder

- `core/` — the hub (server) and the network client. The server is one small
  program: `feather-hub`.
- `web/` — the phone page the hub serves. It is compiled into the program.
- `src/` — the desktop app. **It is not needed on the server** and needs a
  graphical system to build; do not build it here.
- `deploy/install.sh` — the one-command installer and the health/backup tool.
- `DEPLOY.md` — the long human guide. Use it for background, not for typing
  every command by hand.
- `docker-compose.yml`, `Dockerfile` — the Docker path, also driven by the
  installer.

## Before you change anything

Run these and keep the output for your report:

```bash
cat /etc/os-release
uname -m
id -u                                  # 0 means root
systemctl is-system-running            # "running" or "degraded"
ss -ltnp | grep -E ':(80|443|8777)\b' || true
curl -4 -s --max-time 5 https://ifconfig.me || true   # public IPv4
command -v docker && docker info >/dev/null 2>&1 && echo docker-ready || echo no-docker
```

Decisions:

- **Domain.** Ask the person for the name they will use (for example
  `chat.example.com`). Add an **A record** at their registrar pointing to this
  server's public IPv4. HTTPS needs this. If there is no domain yet, the hub
  can run on a plain port for a first look, but phones will get no HTTPS.
- **Sign-ups.** Leave them `open` for the first run so the person can create
  their account. After everyone has joined, switch to `closed`.
- **Mode.** Let `auto` decide. It repairs whatever is already installed; on a
  fresh Debian/Ubuntu server it builds the lighter native service (recommended
  for 1 GB), and elsewhere it uses Docker. Use `--mode docker` if the person
  asks for containers, or `--mode native` to force the smallest footprint.

## Deploy

The default (auto) path:

```bash
cd /opt/feather            # or wherever this folder was copied
sudo bash deploy/install.sh deploy \
  --domain chat.example.com \
  --email you@example.com
```

If the folder is not at `/opt/feather`, that is fine — the script uses its own
location. It installs only what is missing. In native mode it builds the hub,
creates the unprivileged `feather` user, enables a systemd service and installs
Caddy for HTTPS. In Docker mode it builds the two containers instead. Either
way it opens 80/443 in `ufw`/`firewalld` when those are active and seeds an
empty message file so backups work immediately.

Expected shape of the end of the output:

```
Feather is ready.

  Open:     https://chat.example.com
  Data:     /var/lib/feather/hub.json
  Service:  systemctl status feather --no-pager
```

Then verify:

```bash
bash deploy/install.sh status
```

It must end with `Everything asked for is healthy.` and exit 0. If it does not,
read the two commands it prints.

A second, independent check (must print HTTP **200** and a small JSON body):

```bash
curl -fsS --resolve chat.example.com:443:127.0.0.1 https://chat.example.com/api/health
```

Then, depending on the mode:

```bash
# native
curl -fsS http://127.0.0.1:8777/api/health

# Docker (the hub port is not published to the host)
docker compose exec -T hub feather-hub --health
```

Open `https://chat.example.com/` in a browser and confirm the Feather sign-in
page appears. Create the first account, send a test message, then open the page
on a phone (same address) and confirm the message is there. That phone test is
the part a terminal cannot prove.

When the person has their account, close sign-ups:

```bash
sudo bash deploy/install.sh deploy --domain chat.example.com --signup closed
```

The first account can always be created, so this cannot lock an empty server.

## If something fails

| Symptom | What to do |
| --- | --- |
| `Port 80 or 443 is already used` | Something else serves the web. Run `ss -ltnp \| grep -E ':(80\|443)$'`. Stop that service or give Feather a spare port with `--no-proxy --port 8778` (plain HTTP). |
| `Caddy is already serving another site` | Do not overwrite someone's site. Add a Feather block to their Caddyfile, or run `--no-proxy --port 8778` for a first look. |
| `HTTPS is not ready yet` | DNS is missing or still spreading. Check `getent ahosts chat.example.com` against the server's public IP, then `journalctl -u caddy -n 80 --no-pager`. |
| Caddy certificate errors | Port 80/443 blocked by the provider firewall, or DNS points elsewhere. Fix that; do not loop retries. |
| Build runs out of memory | Re-run with `--mode docker` (build happens in a container) or add swap. |
| `systemd is not running here` | This is a container or chroot. Use `--mode docker`. |
| Phone cannot connect | Is the address exactly the HTTPS one? Is port 443 open at the provider? Test from outside with `curl -I https://chat.example.com`. |
| Hub restarts in a loop | `journalctl -u feather -n 80 --no-pager`. Usually the data folder permission or a taken port. |
| Docker certificate loop | `cd <app> && docker compose logs --tail 80 caddy`; check DNS and ports. |

Do not “fix” a failure by publishing 8777 to the internet. The hub speaks plain
HTTP on purpose; Caddy is the public door.

## Everyday commands

```bash
cd /opt/feather
bash deploy/install.sh status                      # is it healthy?
sudo bash deploy/install.sh backup                 # copy the one data file
sudo bash deploy/install.sh restore --file ...json # put a backup back
sudo bash deploy/install.sh deploy --signup closed # change sign-ups
sudo bash deploy/install.sh uninstall              # remove, keep messages
sudo bash deploy/install.sh uninstall --purge      # remove everything
```

Backups go to `/var/backups/feather/` and the newest 14 are kept. The file
contains every message and account; treat it as private. Copy it off the server
now and then.

Native logs: `journalctl -u feather -f`.
Docker logs: `docker compose logs -f hub` (run in the app folder).

## Updating

Replace this folder with the new version (or `git pull` if it came from a
repository), then run the same deploy command again. The data file is outside
the app folder, so messages survive. Take a backup first.

## Limits you must tell the person

- **No end-to-end encryption.** HTTPS protects messages in transit; the server
  can read the stored messages. Suitable for a trusted circle, not strangers.
- **No push notifications.** The phone page updates while open; nothing wakes a
  sleeping phone.
- **No PIN recovery.** A forgotten PIN means a new account.
- **Text first.** In linked chats: text, replies, reactions, read receipts,
  typing and groups. Photos, voice notes and real calls are not included yet.
- **Not carrier RCS.** RCS is an operator service; Feather implements the chat
  features around it, not the carrier protocol.
- The desktop program is for the person's own computer and is not part of the
  server install.

## Report back

Finish with: the public address, which mode was used (native/Docker), whether
`status` passed, whether the phone test passed, where the data and backups are,
and anything you could not verify. Do not claim HTTPS or phone access works if
you did not see a 200 and a message arrive.
