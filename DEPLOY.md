# Feather on a VPS

Feather is ready to run on a small server for a circle of people you know. The
hub is one program, one JSON file and one port. It has no database, no Redis, no
message queue and no cloud account.

## Fast path

Copy this `app` folder to the server (from the folder above it):

```bash
scp -r app root@YOUR_SERVER_IP:/opt/feather
```

On a Debian or Ubuntu server, one command does everything below: build the
hub, create the service, install Caddy and check the result.

```bash
cd /opt/feather
sudo bash deploy/install.sh deploy --domain chat.example.com --email you@example.com
bash deploy/install.sh status
```

It prints the address and stops with a plain error if something is wrong. The
Docker path is `--mode docker`. The installer is safe to run again; it repairs
instead of duplicating.

**If an AI assistant is doing this work on the server, it should read
[AGENTS.md](AGENTS.md) first.** That file is the runbook: preflight checks, the
command, what a good result looks like, a failure table, backups and rollback.

The rest of this guide explains both manual paths, so you can see what the
installer does and change it if you need to. It was exercised on Debian 13 with
systemd and Docker on 2026-11-13; the certificate with a real public domain and
the person's own VPS are still unverified.

The two paths:

- **Native (lightest).** Build the 0.6 MB `feather-hub` program and run it with
  systemd behind Caddy. This is what `auto` picks on a fresh Debian/Ubuntu VPS.
- **Docker.** Two containers: the hub and Caddy, which gets a free HTTPS
  certificate for your domain.

You need a Linux VPS with about 1 GB of memory, a domain name (strongly
recommended), and ports 22, 80 and 443 open. Nothing else needs to be public.

> **Do not publish port 8777 to the internet.** The hub speaks plain HTTP on
> purpose and expects an HTTPS proxy in front of it. Only Caddy's 80 and 443
> are exposed in the files here.

---

## Path A: Docker

### 1. Point your domain at the server

At your domain registrar, create an **A record**:

```
chat.example.com  ->  <your server's public IPv4 address>
```

Wait a few minutes until `ping chat.example.com` shows the server's address.

### 2. Install Docker on the server

Connect over SSH, then:

```bash
curl -fsSL https://get.docker.com | sudo sh
sudo usermod -aG docker "$USER"
```

Log out and back in so the group change applies.

### 3. Copy the app folder to the server

From your own computer, in the folder that contains `app`:

```powershell
scp -r app root@YOUR_SERVER_IP:/opt/feather
```

On macOS or Linux use the same command with `app/`.

### 4. Choose the domain

On the server:

```bash
cd /opt/feather
echo "FEATHER_DOMAIN=chat.example.com" > .env
```

Replace `chat.example.com` with your own name. If you have no domain yet, skip
this line; Caddy will use a self-signed certificate for `localhost`, which only
works for testing.

### 5. Start it

```bash
docker compose up -d --build
```

The first build takes a few minutes. Then open:

```
https://chat.example.com
```

Create the first account, then share that address with the people you want in
your circle. They open it in a phone browser and use **Add to Home screen**.

### 6. Close the door (after everyone has joined)

By default anyone who knows the address can make an account. Once your people
are in, edit `.env`:

```
FEATHER_DOMAIN=chat.example.com
FEATHER_SIGNUP=closed
```

Then:

```bash
docker compose up -d
```

Existing accounts keep working; new sign-ups are refused. The very first
account can always be created, so you cannot lock yourself out of an empty hub.

### Everyday commands

```bash
cd /opt/feather
docker compose logs -f hub        # watch the hub
docker compose ps                 # is it running?
curl -fsS https://chat.example.com/api/health
docker compose up -d --build      # update after replacing the files
docker compose down               # stop (data is kept)
```

### Back up and restore

The history lives in a Docker volume, not in the app folder.

```bash
docker compose exec -T hub cat /data/hub.json > feather-backup-$(date +%F).json
```

Restore:

```bash
docker compose stop hub
cat feather-backup-2026-01-01.json | docker compose exec -T hub sh -c 'cat > /data/hub.json'
docker compose start hub
```

Copy the backup file somewhere else now and then. It contains all messages and
account records, so treat it like a private diary.

---

## Path B: without Docker

### 1. Build the hub

On the server:

```bash
sudo apt update
sudo apt install -y build-essential pkg-config curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

cd /opt/feather
cargo build --release -p feather-core --bin feather-hub
sudo install -m 755 target/release/feather-hub /usr/local/bin/feather-hub
```

### 2. Make a user and a data folder

```bash
sudo useradd --system --home /var/lib/feather --shell /usr/sbin/nologin feather
sudo mkdir -p /var/lib/feather
sudo chown feather:feather /var/lib/feather
```

### 3. Run it as a service

```bash
sudo cp /opt/feather/deploy/feather.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now feather
systemctl status feather
```

The unit listens only on `127.0.0.1:8777`, restarts on failure and can write
only to `/var/lib/feather`.

### 4. Put Caddy in front

Install Caddy from its official repository:

```bash
sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https curl
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
  | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
  | sudo tee /etc/apt/sources.list.d/caddy-stable.list
sudo apt update && sudo apt install -y caddy
```

Replace the contents of `/etc/caddy/Caddyfile` with:

```
chat.example.com {
	encode zstd gzip
	reverse_proxy 127.0.0.1:8777
}
```

Then:

```bash
sudo systemctl reload caddy
```

Caddy obtains and renews the certificate automatically once DNS points at the
server.

### 5. Firewall

```bash
sudo ufw allow OpenSSH
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw enable
```

Do **not** allow 8777. Confirm with `sudo ufw status`.

---

## Settings

| Variable | Default | Meaning |
| --- | --- | --- |
| `FEATHER_PORT` | `8777` | Local port the hub listens on |
| `FEATHER_BIND` | `0.0.0.0` | Local address (use `127.0.0.1` with a proxy on the same machine) |
| `FEATHER_DATA` | user data folder | Path of the one JSON file |
| `FEATHER_SIGNUP` | `open` | `open` or `closed` |

Command-line flags `--port`, `--bind`, `--data`, `--closed` override the
environment. `feather-hub --health` asks a running hub for its health and exits
0 or 1; that is what the container health check uses.

The desktop app joins a server with the address `https://chat.example.com` in
**Feather Link → Join a space**. It speaks HTTPS when given an `https://`
address.

---

## What to expect

- **Size.** The server program is about 0.6 MB and idles at a few megabytes of
  memory plus the message file. The Docker image is about 114 MB because of its
  Debian base; the native install avoids that. A 1 GB VPS is plenty for a
  circle of friends.
- **Backups matter.** Everything is in one JSON file. There is no other copy.
- **Messages are capped.** The hub keeps the newest 8,000 messages in total and
  shows the newest 300 per chat; older ones are dropped to keep it light.
- **No push notifications.** The phone page refreshes while it is open. If the
  phone is asleep or the page is closed, nothing pops up until it is opened.
- **One hub.** Accounts live on your server. There is no federation and no
  import from another messenger.
- **No PIN recovery.** A forgotten PIN means making a new account. That is the
  price of not collecting an email address.

## Security, plainly

Good:

- HTTPS between people's browsers and the server, with a real certificate.
- PINs are stored as salted, many-round SHA-256 hashes, not plain text.
- Login attempts are limited per address; ten wrong tries pause that address
  for ten minutes.
- Session tokens are stored on the server only as digests.
- The hub runs as an unprivileged user and writes only its data folder.
- No third-party analytics, ads or external calls from the page.

Not there yet:

- **No end-to-end encryption.** The server can read messages. HTTPS protects
  them in transit, not from the server itself.
- The hop from Caddy to the hub is plain HTTP on loopback or a private Docker
  network. That is normal for a self-hosted small app, but it is not E2E.
- Tokens do not expire on a timer. Signing out on a phone forgets the token
  there; removing a person entirely is not a feature yet.
- Anyone with the address can try to sign up until you set
  `FEATHER_SIGNUP=closed`. Do that after your circle joins.
- No media, voice notes or real calls in linked chats.

For a trusted circle this is a reasonable small server. For strangers, public
communities or sensitive material, it is not enough yet.
