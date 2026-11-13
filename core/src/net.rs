//! Feather Link: a tiny message hub plus the client both the desktop app and
//! the phone web app talk to. No cloud, no accounts beyond a name and a PIN.
//! Everything is kept in one JSON file on the hub.
//!
//! The hub listens on plain HTTP so it stays small and dependency-free. On a
//! server you put a TLS proxy (Caddy, nginx) in front of it; the desktop client
//! understands `https://` addresses as well.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The version the health endpoint reports and the server logs on start.
pub const VERSION: &str = "1.1.0";

// ---------------------------------------------------------------- data model

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub pin: String,
    pub color: u32,
    pub created: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reaction {
    pub user: String,
    pub emoji: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredMessage {
    pub id: u64,
    pub conv: u64,
    pub from: String,
    pub text: String,
    pub at: i64,
    #[serde(default)]
    pub reply_to: Option<u64>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Conversation {
    pub id: u64,
    pub title: String,
    pub group: bool,
    pub members: Vec<String>,
    pub created: i64,
    #[serde(default)]
    pub last_read: HashMap<String, u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Store {
    #[serde(default)]
    pub users: Vec<User>,
    #[serde(default)]
    pub convs: Vec<Conversation>,
    #[serde(default)]
    pub msgs: Vec<StoredMessage>,
    #[serde(default)]
    pub tokens: HashMap<String, String>,
    #[serde(default)]
    pub next_id: u64,
    #[serde(default)]
    pub next_msg: u64,
    #[serde(default)]
    pub rev: u64,
}

impl Store {
    fn new() -> Self {
        Self {
            next_id: 1,
            next_msg: 1,
            rev: 1,
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Me {
    pub id: String,
    pub name: String,
    pub color: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub color: u32,
    pub online: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReactionView {
    pub user: String,
    pub name: String,
    pub emoji: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MsgView {
    pub id: u64,
    pub conv: u64,
    pub from: String,
    pub from_name: String,
    pub text: String,
    pub at: i64,
    pub reply_to: Option<u64>,
    pub reply_name: Option<String>,
    pub reply_text: Option<String>,
    pub reactions: Vec<ReactionView>,
    pub read: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConvView {
    pub id: u64,
    pub title: String,
    pub group: bool,
    pub members: Vec<Person>,
    pub messages: Vec<MsgView>,
    pub unread: u32,
    pub typing: Vec<String>,
    pub last_at: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub rev: u64,
    pub me: Option<Me>,
    pub people: Vec<Person>,
    pub convs: Vec<ConvView>,
    pub hub_name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AuthReply {
    pub token: String,
    pub me: Me,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ErrorReply {
    pub error: Option<String>,
}

// ------------------------------------------------------------------- helpers

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ------------------------------------------------------------- PIN hashing
//
// Version 1 stored a bare 64-bit digest, which is far too cheap to guess if a
// hub's data file ever leaked. Version 2 uses a random salt and many rounds of
// SHA-256. Old records still sign in and are upgraded in place on first use, so
// an existing hub keeps working after an update.

const PIN_ROUNDS: u32 = 20_000;
const MAX_FAILS: u32 = 10;
const FAIL_WINDOW: Duration = Duration::from_secs(10 * 60);

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = (bytes[i] as char).to_digit(16)?;
        let lo = (bytes[i + 1] as char).to_digit(16)?;
        out.push((hi * 16 + lo) as u8);
        i += 2;
    }
    Some(out)
}

fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// The old, weak digest, kept only so existing accounts still work.
fn legacy_digest(name: &str, pin: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in name.to_lowercase().bytes().chain(pin.bytes()) {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}

fn new_salt() -> [u8; 16] {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut out = [0u8; 16];
    for chunk in out.chunks_mut(8) {
        let mut h = RandomState::new().build_hasher();
        h.write_u128(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        );
        chunk.copy_from_slice(&h.finish().to_le_bytes());
    }
    out
}

fn hash_pin(name: &str, salt: &[u8], pin: &str) -> String {
    let mut first = Sha256::new();
    first.update(b"feather-pin-v2");
    first.update(salt);
    first.update(name.to_lowercase().as_bytes());
    first.update(pin.as_bytes());
    let mut h = first.finalize();
    for round in 0..PIN_ROUNDS {
        let mut d = Sha256::new();
        d.update(h);
        d.update(salt);
        d.update(pin.as_bytes());
        d.update(round.to_le_bytes());
        h = d.finalize();
    }
    hex(&h)
}

fn pin_record(name: &str, pin: &str) -> String {
    let salt = new_salt();
    format!("s2${}${}", hex(&salt), hash_pin(name, &salt, pin))
}

fn pin_matches(stored: &str, name: &str, pin: &str) -> bool {
    if let Some(rest) = stored.strip_prefix("s2$") {
        let Some((salt_hex, want)) = rest.split_once('$') else {
            return false;
        };
        let Some(salt) = unhex(salt_hex) else {
            return false;
        };
        let got = hash_pin(name, &salt, pin);
        same_bytes(want.as_bytes(), got.as_bytes())
    } else {
        same_bytes(stored.as_bytes(), legacy_digest(name, pin).as_bytes())
    }
}

fn token_key(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

/// One small bucket per address so a stranger cannot guess PINs all day.
fn too_many_fails(inner: &Inner, peer: &str) -> bool {
    inner
        .fails
        .get(peer)
        .map(|(count, at)| *count >= MAX_FAILS && at.elapsed() < FAIL_WINDOW)
        .unwrap_or(false)
}

fn note_fail(inner: &mut Inner, peer: &str) {
    let entry = inner
        .fails
        .entry(peer.to_owned())
        .or_insert((0, Instant::now()));
    if entry.1.elapsed() >= FAIL_WINDOW {
        *entry = (0, Instant::now());
    }
    entry.0 += 1;
    entry.1 = Instant::now();
}

fn clear_fails(inner: &mut Inner, peer: &str) {
    inner.fails.remove(peer);
}

fn clean_name(name: &str) -> String {
    name.trim().chars().take(28).collect()
}

fn color_for(index: usize) -> u32 {
    const COLORS: [u32; 8] = [
        0x347868, 0x5b8dd6, 0xd47a5a, 0x8a6fc4, 0xc99a3e, 0x4a9b8e, 0xc4668a, 0x6c8f4a,
    ];
    COLORS[index % COLORS.len()]
}

pub fn default_data_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("Feather").join("hub.json");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("feather")
            .join("hub.json");
    }
    PathBuf::from("feather-hub.json")
}

pub fn local_addresses() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = sock.local_addr() {
                let ip = addr.ip();
                if !ip.is_loopback() && ip.is_ipv4() {
                    out.push(ip.to_string());
                }
            }
        }
    }
    if out.is_empty() {
        out.push("127.0.0.1".to_owned());
    }
    out
}

pub fn share_host(port: u16) -> String {
    format!("{}:{}", local_addresses()[0], port)
}

/// The Windows Firewall rule Feather adds when the host asks for it. Keeping
/// the port in the name means a second hub on another port gets its own rule.
pub fn firewall_rule_name(port: u16) -> String {
    format!("Feather Chat hub {port}")
}

/// True when Windows already has the inbound rule for this hub port. On other
/// systems there is no equivalent step, so the answer is always true.
#[cfg(target_os = "windows")]
pub fn firewall_rule_exists(port: u16) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let name = format!("name=\"{}\"", firewall_rule_name(port));
    let out = std::process::Command::new("netsh")
        .args(["advfirewall", "firewall", "show", "rule", &name, "dir=in"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    match out {
        Ok(o) => {
            o.status.success()
                && String::from_utf8_lossy(&o.stdout)
                    .to_lowercase()
                    .contains("feather chat hub")
        }
        Err(_) => false,
    }
}

#[cfg(not(target_os = "windows"))]
pub fn firewall_rule_exists(_port: u16) -> bool {
    true
}

/// Asks Windows (through its own UAC prompt) to let phones reach this hub
/// port. Only that TCP port is opened, on every network profile, so the same
/// rule still works when Windows treats the Wi-Fi as public.
#[cfg(target_os = "windows")]
pub fn allow_firewall(port: u16) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let add = format!(
        "advfirewall firewall add rule name=\"{}\" dir=in action=allow protocol=TCP localport={} profile=any",
        firewall_rule_name(port),
        port
    );
    let script = format!(
        "$ErrorActionPreference='Stop'; try {{ $p = Start-Process -FilePath 'netsh.exe' -ArgumentList '{add}' -Verb RunAs -PassThru -Wait -WindowStyle Hidden; exit $p.ExitCode }} catch {{ exit 1 }}"
    );
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Could not reach Windows Firewall: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim();
        Err(if err.is_empty() {
            "Windows did not allow the change. If a prompt appeared, choose Yes.".to_owned()
        } else {
            format!("Windows did not allow the change: {err}")
        })
    }
}

#[cfg(not(target_os = "windows"))]
pub fn allow_firewall(_port: u16) -> Result<(), String> {
    Ok(())
}

/// Accepts "192.168.1.5:8777", "https://chat.example.com" or a bare host and
/// returns a full address with its scheme. A bare address keeps the original
/// same-Wi-Fi default of port 8777; `https://` means the usual 443.
pub fn normalize_code(input: &str) -> String {
    let mut s = input.trim().to_owned();
    let mut secure = false;
    if let Some(rest) = s.strip_prefix("http://") {
        s = rest.to_owned();
    } else if let Some(rest) = s.strip_prefix("https://") {
        s = rest.to_owned();
        secure = true;
    }
    if let Some(i) = s.find('/') {
        s.truncate(i);
    }
    s.retain(|c| c != ' ');
    if s.is_empty() {
        return s;
    }
    if !address_has_port(&s) && !secure {
        s.push_str(":8777");
    }
    if secure {
        format!("https://{s}")
    } else {
        format!("http://{s}")
    }
}

fn address_has_port(address: &str) -> bool {
    if address.starts_with('[') {
        return address.rfind("]:").is_some();
    }
    match address.rfind(':') {
        Some(i) => address[i + 1..].chars().all(|c| c.is_ascii_digit()),
        None => false,
    }
}

// ------------------------------------------------------------------ hub side

struct Inner {
    store: Store,
    seen: HashMap<String, Instant>,
    typing: HashMap<String, i64>,
    fails: HashMap<String, (u32, Instant)>,
}

struct Shared {
    inner: Mutex<Inner>,
    data_path: PathBuf,
    started: Instant,
    signup_open: bool,
}

pub struct Hub {
    pub port: u16,
    pub data_path: PathBuf,
    _shared: Arc<Shared>,
    shutdown: Arc<AtomicBool>,
}

/// How the server should run. The desktop app keeps the friendly fallback to
/// the next free port; a server run takes the exact port so a supervisor can
/// see immediately when the port is already busy.
#[derive(Clone, Debug)]
pub struct HubConfig {
    pub bind: String,
    pub port: u16,
    pub data_path: PathBuf,
    pub strict_port: bool,
    pub signup_open: bool,
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0".to_owned(),
            port: 8777,
            data_path: default_data_path(),
            strict_port: false,
            signup_open: true,
        }
    }
}

impl HubConfig {
    /// Reads the same names the deployment files use, so `docker run`,
    /// systemd and a shell all configure the hub the same way.
    pub fn from_env() -> Self {
        let mut cfg = HubConfig::default();
        if let Ok(bind) = std::env::var("FEATHER_BIND") {
            if !bind.trim().is_empty() {
                cfg.bind = bind.trim().to_owned();
            }
        }
        if let Some(port) = std::env::var("FEATHER_PORT")
            .ok()
            .and_then(|p| p.trim().parse::<u16>().ok())
        {
            cfg.port = port;
        }
        if let Ok(path) = std::env::var("FEATHER_DATA") {
            if !path.trim().is_empty() {
                cfg.data_path = PathBuf::from(path.trim());
            }
        }
        if let Ok(mode) = std::env::var("FEATHER_SIGNUP") {
            cfg.signup_open = !matches!(
                mode.trim().to_ascii_lowercase().as_str(),
                "closed" | "off" | "false" | "0" | "invite"
            );
        }
        cfg
    }
}

impl Hub {
    /// Starts the hub on `port` (or the next free port) and returns once it is
    /// accepting connections.
    pub fn start(port: u16, data_path: PathBuf) -> Result<Hub, String> {
        Hub::start_with(HubConfig {
            port,
            data_path,
            ..HubConfig::default()
        })
    }

    /// Server entry point: exact port, chosen bind address, configurable
    /// signups. Returns once the listener is accepting connections.
    pub fn start_with(cfg: HubConfig) -> Result<Hub, String> {
        ensure_storage(&cfg.data_path)?;
        let store = load_store(&cfg.data_path);
        let shared = Arc::new(Shared {
            inner: Mutex::new(Inner {
                store,
                seen: HashMap::new(),
                typing: HashMap::new(),
                fails: HashMap::new(),
            }),
            data_path: cfg.data_path.clone(),
            started: Instant::now(),
            signup_open: cfg.signup_open,
        });

        let mut chosen = None;
        let mut last_err = String::new();
        let attempts = if cfg.strict_port { 1 } else { 12 };
        for p in cfg.port..cfg.port.saturating_add(attempts) {
            let addr = format!("{}:{p}", cfg.bind);
            match TcpListener::bind(&addr) {
                Ok(listener) => {
                    // Port 0 asks the system for a free port; report the real one.
                    let actual = listener.local_addr().map(|a| a.port()).unwrap_or(p);
                    chosen = Some((actual, listener));
                    break;
                }
                Err(e) => last_err = e.to_string(),
            }
        }
        let (port, listener) =
            chosen.ok_or_else(|| format!("Could not listen on {}:{}: {last_err}", cfg.bind, cfg.port))?;

        let _ = listener.set_nonblocking(true);
        let shared_for_thread = shared.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_for_thread = shutdown.clone();
        std::thread::spawn(move || {
            while !shutdown_for_thread.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, peer)) => {
                        let _ = stream.set_nonblocking(false);
                        let shared = shared_for_thread.clone();
                        let ip = peer.ip();
                        std::thread::spawn(move || {
                            let _ = serve_one(stream, &shared, ip);
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(40));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(120)),
                }
            }
        });

        Ok(Hub {
            port,
            data_path: cfg.data_path,
            _shared: shared,
            shutdown,
        })
    }

    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

impl Drop for Hub {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Fails early, with a plain sentence, when the data folder cannot be written.
/// A server that starts and only then fails to save would lose messages.
fn ensure_storage(path: &Path) -> Result<(), String> {
    let Some(dir) = path.parent() else {
        return Ok(());
    };
    if dir.as_os_str().is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Could not create the data folder {}: {e}", dir.display()))?;
    let probe = dir.join(".feather-write-test");
    std::fs::write(&probe, b"ok")
        .map_err(|e| format!("The data folder {} is not writable: {e}", dir.display()))?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

fn load_store(path: &Path) -> Store {
    if let Ok(text) = std::fs::read_to_string(path) {
        if let Ok(store) = serde_json::from_str::<Store>(&text) {
            return store;
        }
    }
    Store::new()
}

fn save_store(shared: &Shared) {
    let text = {
        let inner = shared.inner.lock().unwrap();
        match serde_json::to_string_pretty(&inner.store) {
            Ok(t) => t,
            Err(_) => return,
        }
    };
    if let Some(parent) = shared.data_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = shared.data_path.with_extension("tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, &shared.data_path);
    }
}

struct Request {
    method: String,
    path: String,
    token: Option<String>,
    body: String,
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    stream.set_read_timeout(Some(Duration::from_secs(6))).ok()?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end;
    loop {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = find_bytes(&buf, b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
        if buf.len() > 64 * 1024 {
            return None;
        }
    }

    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = head.split("\r\n");
    let first = lines.next()?;
    let mut parts = first.split_whitespace();
    let method = parts.next()?.to_owned();
    let path = parts.next()?.to_owned();

    let mut content_length = 0usize;
    let mut token = None;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim().to_ascii_lowercase();
            let v = v.trim();
            if k == "content-length" {
                content_length = v.parse().unwrap_or(0);
            } else if k == "authorization" {
                token = v
                    .strip_prefix("Bearer ")
                    .or_else(|| v.strip_prefix("bearer "))
                    .map(|s| s.trim().to_owned());
            }
        }
    }
    if content_length > 512 * 1024 {
        return None;
    }

    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);

    Some(Request {
        method,
        path,
        token,
        body: String::from_utf8_lossy(&body).to_string(),
    })
}

fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn serve_one(mut stream: TcpStream, shared: &Shared, peer: IpAddr) -> std::io::Result<()> {
    let req = match read_request(&mut stream) {
        Some(r) => r,
        None => return Ok(()),
    };
    let peer = peer.to_string();

    let path = req.path.split('?').next().unwrap_or("/").to_owned();
    if req.method == "OPTIONS" {
        return write_response(&mut stream, "204 No Content", "text/plain", b"");
    }

    let (status, ctype, body) = match (req.method.as_str(), path.as_str()) {
        ("GET", "/") | ("GET", "/index.html") => {
            ("200 OK", "text/html; charset=utf-8", web_index().as_bytes().to_vec())
        }
        ("GET", "/mobile.css") => (
            "200 OK",
            "text/css; charset=utf-8",
            web_css().as_bytes().to_vec(),
        ),
        ("GET", "/mobile.js") => (
            "200 OK",
            "application/javascript; charset=utf-8",
            web_js().as_bytes().to_vec(),
        ),
        ("GET", "/manifest.webmanifest") => (
            "200 OK",
            "application/manifest+json; charset=utf-8",
            web_manifest().as_bytes().to_vec(),
        ),
        ("GET", "/sw.js") => (
            "200 OK",
            "application/javascript; charset=utf-8",
            web_sw().as_bytes().to_vec(),
        ),
        ("GET", "/icon.svg") => (
            "200 OK",
            "image/svg+xml",
            web_icon().as_bytes().to_vec(),
        ),
        ("GET", "/favicon.ico") => ("204 No Content", "image/x-icon", Vec::new()),
        ("GET", "/api/health") | ("GET", "/healthz") => api_health(shared),
        ("GET", "/api/state") => api_state(shared, req.token.as_deref()),
        ("POST", "/api/signup") => api_signup(shared, &req.body, &peer),
        ("POST", "/api/login") => api_login(shared, &req.body, &peer),
        ("POST", "/api/message") => api_message(shared, req.token.as_deref(), &req.body),
        ("POST", "/api/read") => api_read(shared, req.token.as_deref(), &req.body),
        ("POST", "/api/typing") => api_typing(shared, req.token.as_deref(), &req.body),
        ("POST", "/api/conversation") => {
            api_conversation(shared, req.token.as_deref(), &req.body)
        }
        ("POST", "/api/react") => api_react(shared, req.token.as_deref(), &req.body),
        _ => (
            "404 Not Found",
            "application/json",
            br#"{"error":"Not found"}"#.to_vec(),
        ),
    };

    write_response(&mut stream, status, ctype, &body)
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    ctype: &str,
    body: &[u8],
) -> std::io::Result<()> {
    // No wildcard CORS: the phone page is served from this same origin, and
    // the desktop client is not a browser. A stranger's web page should not be
    // able to read a local hub's answers.
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn json_ok(body: &str) -> (&'static str, &'static str, Vec<u8>) {
    ("200 OK", "application/json", body.as_bytes().to_vec())
}

fn json_err(message: &str) -> (&'static str, &'static str, Vec<u8>) {
    (
        "400 Bad Request",
        "application/json",
        serde_json::json!({ "error": message })
            .to_string()
            .into_bytes(),
    )
}

fn json_error_value(message: &str) -> Vec<u8> {
    serde_json::json!({ "error": message })
        .to_string()
        .into_bytes()
}

fn auth_user(inner: &mut Inner, token: Option<&str>) -> Option<String> {
    let token = token?;
    // New tokens are stored only as a digest, so a copied data file does not
    // hand over live sessions. The raw lookup keeps older hubs working.
    let user = inner
        .store
        .tokens
        .get(&token_key(token))
        .or_else(|| inner.store.tokens.get(token))?
        .clone();
    inner.seen.insert(user.clone(), Instant::now());
    Some(user)
}

fn me_of(inner: &Inner, id: &str) -> Option<Me> {
    inner.store.users.iter().find(|u| u.id == id).map(|u| Me {
        id: u.id.clone(),
        name: u.name.clone(),
        color: u.color,
    })
}

fn display_name(inner: &Inner, id: &str) -> String {
    inner
        .store
        .users
        .iter()
        .find(|u| u.id == id)
        .map(|u| u.name.clone())
        .unwrap_or_else(|| "Someone".to_owned())
}

fn build_snapshot(inner: &mut Inner, me: &str) -> Snapshot {
    let store = &inner.store;
    let now = Instant::now();
    let people: Vec<Person> = store
        .users
        .iter()
        .map(|u| Person {
            id: u.id.clone(),
            name: u.name.clone(),
            color: u.color,
            online: inner
                .seen
                .get(&u.id)
                .map(|t| now.duration_since(*t).as_secs() < 30)
                .unwrap_or(false),
        })
        .collect();

    let mut convs = Vec::new();
    for conv in store.convs.iter().filter(|c| c.members.contains(&me.to_owned())) {
        let mut msgs: Vec<&StoredMessage> =
            store.msgs.iter().filter(|m| m.conv == conv.id).collect();
        msgs.sort_by_key(|m| m.id);
        if msgs.len() > 300 {
            let keep = msgs.len() - 300;
            msgs.drain(0..keep);
        }

        let last_read = conv.last_read.get(me).copied().unwrap_or(0);
        let unread = msgs
            .iter()
            .filter(|m| m.from != me && m.id > last_read)
            .count() as u32;

        let members: Vec<Person> = conv
            .members
            .iter()
            .filter_map(|id| people.iter().find(|p| &p.id == id).cloned())
            .collect();

        let title = if conv.group {
            conv.title.clone()
        } else {
            conv.members
                .iter()
                .find(|id| id.as_str() != me)
                .map(|id| display_name(inner, id))
                .unwrap_or_else(|| "Saved notes".to_owned())
        };

        let typing: Vec<String> = inner
            .typing
            .iter()
            .filter(|(k, t)| {
                k.starts_with(&format!("{}:", conv.id))
                    && now_ms() - **t < 4500
                    && !k.ends_with(&format!(":{me}"))
            })
            .map(|(k, _)| {
                let uid = k.split(':').nth(1).unwrap_or("");
                display_name(inner, uid)
            })
            .collect();

        let views: Vec<MsgView> = msgs
            .iter()
            .map(|m| {
                let reply = m.reply_to.and_then(|rid| {
                    store.msgs.iter().find(|r| r.id == rid).map(|r| {
                        (display_name(inner, &r.from), r.text.clone())
                    })
                });
                let read = m.from == me
                    && conv.members.iter().all(|id| {
                        id == me
                            || conv.last_read.get(id).copied().unwrap_or(0) >= m.id
                    });
                MsgView {
                    id: m.id,
                    conv: m.conv,
                    from: m.from.clone(),
                    from_name: display_name(inner, &m.from),
                    text: m.text.clone(),
                    at: m.at,
                    reply_to: m.reply_to,
                    reply_name: reply.as_ref().map(|r| r.0.clone()),
                    reply_text: reply.as_ref().map(|r| r.1.clone()),
                    reactions: m
                        .reactions
                        .iter()
                        .map(|r| ReactionView {
                            user: r.user.clone(),
                            name: display_name(inner, &r.user),
                            emoji: r.emoji.clone(),
                        })
                        .collect(),
                    read,
                }
            })
            .collect();

        convs.push(ConvView {
            id: conv.id,
            title,
            group: conv.group,
            members,
            last_at: views.last().map(|m| m.at).unwrap_or(conv.created),
            unread,
            typing,
            messages: views,
        });
    }
    convs.sort_by(|a, b| b.last_at.cmp(&a.last_at));

    Snapshot {
        rev: store.rev,
        me: me_of(inner, me),
        people,
        convs,
        hub_name: "Feather Hub".to_owned(),
    }
}

fn api_health(shared: &Shared) -> (&'static str, &'static str, Vec<u8>) {
    let inner = shared.inner.lock().unwrap();
    let body = serde_json::json!({
        "ok": true,
        "name": "Feather Hub",
        "version": VERSION,
        "uptime_seconds": shared.started.elapsed().as_secs(),
        "users": inner.store.users.len(),
        "chats": inner.store.convs.len(),
        "messages": inner.store.msgs.len(),
        "signup": if shared.signup_open { "open" } else { "closed" }
    });
    json_ok(&body.to_string())
}

fn api_state(shared: &Shared, token: Option<&str>) -> (&'static str, &'static str, Vec<u8>) {
    let mut inner = shared.inner.lock().unwrap();
    let me = match auth_user(&mut inner, token) {
        Some(m) => m,
        None => {
            return (
                "401 Unauthorized",
                "application/json",
                json_error_value("Please sign in again."),
            )
        }
    };
    let snap = build_snapshot(&mut inner, &me);
    let body = serde_json::to_string(&snap).unwrap_or_else(|_| "{}".to_owned());
    json_ok(&body)
}

#[derive(Deserialize)]
struct Creds {
    name: String,
    pin: String,
}

fn new_token(store: &mut Store, user: &str) -> String {
    let id = store.next_id;
    store.next_id += 1;
    format!("{}-{}-{}", user, now(), id)
}

fn api_signup(shared: &Shared, body: &str, peer: &str) -> (&'static str, &'static str, Vec<u8>) {
    let creds: Creds = match serde_json::from_str(body) {
        Ok(c) => c,
        Err(_) => return json_err("That didn't look like a name and PIN."),
    };
    let name = clean_name(&creds.name);
    let pin = creds.pin.trim().to_owned();
    if name.len() < 2 {
        return json_err("Pick a name with at least 2 letters.");
    }
    if pin.len() < 4 {
        return json_err("Your PIN needs at least 4 characters.");
    }

    let mut inner = shared.inner.lock().unwrap();
    if too_many_fails(&inner, peer) {
        return (
            "429 Too Many Requests",
            "application/json",
            json_error_value("Too many tries. Wait ten minutes and try again."),
        );
    }
    if !shared.signup_open && !inner.store.users.is_empty() {
        return json_err("This space isn't taking new accounts. Ask the host.");
    }
    if inner
        .store
        .users
        .iter()
        .any(|u| u.name.eq_ignore_ascii_case(&name))
    {
        return json_err("That name is taken. Try signing in instead.");
    }

    let id = format!("u{}", inner.store.next_id);
    inner.store.next_id += 1;
    let color = color_for(inner.store.users.len());
    let user = User {
        id: id.clone(),
        name: name.clone(),
        pin: pin_record(&name, &pin),
        color,
        created: now(),
    };
    inner.store.users.push(user);
    let token = new_token(&mut inner.store, &id);
    inner.store.tokens.insert(token_key(&token), id.clone());
    clear_fails(&mut inner, peer);
    inner.store.rev += 1;
    let reply = serde_json::json!({
        "token": token,
        "me": { "id": id, "name": name, "color": color }
    });
    drop(inner);
    save_store(shared);
    json_ok(&reply.to_string())
}

fn api_login(shared: &Shared, body: &str, peer: &str) -> (&'static str, &'static str, Vec<u8>) {
    let creds: Creds = match serde_json::from_str(body) {
        Ok(c) => c,
        Err(_) => return json_err("That didn't look like a name and PIN."),
    };
    let name = clean_name(&creds.name);
    let pin = creds.pin.trim().to_owned();

    let mut inner = shared.inner.lock().unwrap();
    if too_many_fails(&inner, peer) {
        return (
            "429 Too Many Requests",
            "application/json",
            json_error_value("Too many tries. Wait ten minutes and try again."),
        );
    }
    let found = inner
        .store
        .users
        .iter()
        .find(|u| u.name.eq_ignore_ascii_case(&name))
        .cloned();
    let mut user = match found {
        Some(u) if pin_matches(&u.pin, &u.name, &pin) => u,
        _ => {
            note_fail(&mut inner, peer);
            return json_err("That name and PIN didn't match.");
        }
    };
    clear_fails(&mut inner, peer);

    // Quietly move an old, weak PIN record to the modern one.
    let upgraded = !user.pin.starts_with("s2$");
    if upgraded {
        let fresh = pin_record(&user.name, &pin);
        if let Some(stored) = inner.store.users.iter_mut().find(|u| u.id == user.id) {
            stored.pin = fresh.clone();
        }
        user.pin = fresh;
    }

    let token = new_token(&mut inner.store, &user.id);
    inner.store.tokens.insert(token_key(&token), user.id.clone());
    inner.seen.insert(user.id.clone(), Instant::now());
    let reply = serde_json::json!({
        "token": token,
        "me": { "id": user.id, "name": user.name, "color": user.color }
    });
    drop(inner);
    save_store(shared);
    json_ok(&reply.to_string())
}

#[derive(Deserialize)]
struct SendReq {
    conv: u64,
    text: String,
    #[serde(default)]
    reply_to: Option<u64>,
}

fn api_message(shared: &Shared, token: Option<&str>, body: &str) -> (&'static str, &'static str, Vec<u8>) {
    let req: SendReq = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(_) => return json_err("The message was empty."),
    };
    let text = req.text.trim().to_owned();
    if text.is_empty() {
        return json_err("Write something first.");
    }
    if text.len() > 4000 {
        return json_err("That message is too long.");
    }

    let mut inner = shared.inner.lock().unwrap();
    let me = match auth_user(&mut inner, token) {
        Some(m) => m,
        None => return json_err("Please sign in again."),
    };
    let conv = match inner.store.convs.iter().find(|c| c.id == req.conv) {
        Some(c) if c.members.contains(&me) => c.clone(),
        _ => return json_err("That chat isn't available."),
    };

    let id = inner.store.next_msg;
    inner.store.next_msg += 1;
    inner.store.msgs.push(StoredMessage {
        id,
        conv: conv.id,
        from: me.clone(),
        text,
        at: now(),
        reply_to: req.reply_to,
        reactions: Vec::new(),
    });
    if inner.store.msgs.len() > 8000 {
        let drop = inner.store.msgs.len() - 8000;
        inner.store.msgs.drain(0..drop);
    }
    if let Some(c) = inner.store.convs.iter_mut().find(|c| c.id == conv.id) {
        c.last_read.insert(me, id);
    }
    inner.store.rev += 1;
    drop(inner);
    save_store(shared);
    json_ok(r#"{"ok":true}"#)
}

#[derive(Deserialize)]
struct ConvReq {
    #[serde(default)]
    members: Vec<String>,
    #[serde(default)]
    title: String,
}

fn api_conversation(
    shared: &Shared,
    token: Option<&str>,
    body: &str,
) -> (&'static str, &'static str, Vec<u8>) {
    let req: ConvReq = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(_) => return json_err("Pick someone to message."),
    };
    let mut inner = shared.inner.lock().unwrap();
    let me = match auth_user(&mut inner, token) {
        Some(m) => m,
        None => return json_err("Please sign in again."),
    };

    let mut members = vec![me.clone()];
    for m in req.members {
        if m != me && inner.store.users.iter().any(|u| u.id == m) && !members.contains(&m) {
            members.push(m);
        }
    }
    if members.len() < 2 {
        return json_err("Pick at least one person.");
    }
    let group = members.len() > 2;

    // Reuse an existing direct chat instead of making duplicates.
    if !group {
        if let Some(existing) = inner.store.convs.iter().find(|c| {
            !c.group
                && c.members.len() == 2
                && members.iter().all(|m| c.members.contains(m))
        }) {
            let reply = serde_json::json!({ "id": existing.id });
            return json_ok(&reply.to_string());
        }
    }

    let title = if group {
        let t = req.title.trim();
        if t.is_empty() {
            members
                .iter()
                .filter(|m| *m != &me)
                .map(|m| display_name(&inner, m))
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            t.chars().take(40).collect()
        }
    } else {
        String::new()
    };

    let id = inner.store.next_id;
    inner.store.next_id += 1;
    inner.store.convs.push(Conversation {
        id,
        title,
        group,
        members,
        created: now(),
        last_read: HashMap::new(),
    });
    inner.store.rev += 1;
    let reply = serde_json::json!({ "id": id });
    drop(inner);
    save_store(shared);
    json_ok(&reply.to_string())
}

#[derive(Deserialize)]
struct ConvId {
    conv: u64,
}

fn api_read(shared: &Shared, token: Option<&str>, body: &str) -> (&'static str, &'static str, Vec<u8>) {
    let req: ConvId = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(_) => return json_err("No chat given."),
    };
    let mut inner = shared.inner.lock().unwrap();
    let me = match auth_user(&mut inner, token) {
        Some(m) => m,
        None => return json_err("Please sign in again."),
    };
    let latest = inner
        .store
        .msgs
        .iter()
        .filter(|m| m.conv == req.conv)
        .map(|m| m.id)
        .max()
        .unwrap_or(0);
    let mut changed = false;
    if let Some(c) = inner
        .store
        .convs
        .iter_mut()
        .find(|c| c.id == req.conv && c.members.contains(&me))
    {
        let current = c.last_read.get(&me).copied().unwrap_or(0);
        if latest > current {
            c.last_read.insert(me, latest);
            inner.store.rev += 1;
            changed = true;
        }
    }
    drop(inner);
    if changed {
        save_store(shared);
    }
    json_ok(r#"{"ok":true}"#)
}

fn api_typing(shared: &Shared, token: Option<&str>, body: &str) -> (&'static str, &'static str, Vec<u8>) {
    let req: ConvId = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(_) => return json_err("No chat given."),
    };
    let mut inner = shared.inner.lock().unwrap();
    let me = match auth_user(&mut inner, token) {
        Some(m) => m,
        None => return json_err("Please sign in again."),
    };
    if inner
        .store
        .convs
        .iter()
        .any(|c| c.id == req.conv && c.members.contains(&me))
    {
        inner.typing.insert(format!("{}:{}", req.conv, me), now_ms());
    }
    json_ok(r#"{"ok":true}"#)
}

#[derive(Deserialize)]
struct ReactReq {
    message: u64,
    emoji: String,
}

fn api_react(shared: &Shared, token: Option<&str>, body: &str) -> (&'static str, &'static str, Vec<u8>) {
    let req: ReactReq = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(_) => return json_err("No message given."),
    };
    let emoji: String = req.emoji.chars().take(4).collect();
    if emoji.trim().is_empty() {
        return json_err("No reaction given.");
    }
    let mut inner = shared.inner.lock().unwrap();
    let me = match auth_user(&mut inner, token) {
        Some(m) => m,
        None => return json_err("Please sign in again."),
    };
    let allowed = inner.store.msgs.iter().find(|m| m.id == req.message).map(|m| {
        inner
            .store
            .convs
            .iter()
            .any(|c| c.id == m.conv && c.members.contains(&me))
    });
    if allowed != Some(true) {
        return json_err("That message isn't available.");
    }
    if let Some(msg) = inner.store.msgs.iter_mut().find(|m| m.id == req.message) {
        if let Some(pos) = msg
            .reactions
            .iter()
            .position(|r| r.user == me && r.emoji == emoji)
        {
            msg.reactions.remove(pos);
        } else {
            msg.reactions.retain(|r| r.user != me);
            msg.reactions.push(Reaction {
                user: me,
                emoji,
            });
        }
        inner.store.rev += 1;
    }
    drop(inner);
    save_store(shared);
    json_ok(r#"{"ok":true}"#)
}

// ------------------------------------------------------------------- client

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedLink {
    pub base: String,
    pub token: String,
    pub me: Me,
    #[serde(default)]
    pub host_port: Option<u16>,
}

pub fn saved_link_path() -> PathBuf {
    default_data_path().with_file_name("client.json")
}

pub fn load_saved_link() -> Option<SavedLink> {
    let text = std::fs::read_to_string(saved_link_path()).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save_saved_link(saved: &SavedLink) {
    let path = saved_link_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(saved) {
        let _ = std::fs::write(path, text);
    }
}

pub fn clear_saved_link() {
    let _ = std::fs::remove_file(saved_link_path());
}

#[derive(Clone)]
pub struct HubClient {
    pub base: String,
    pub token: String,
    pub me: Me,
}

impl HubClient {
    pub fn from_saved(saved: &SavedLink) -> Result<HubClient, String> {
        let client = HubClient {
            base: saved.base.clone(),
            token: saved.token.clone(),
            me: saved.me.clone(),
        };
        client.snapshot()?;
        Ok(client)
    }

    pub fn signup(base: &str, name: &str, pin: &str) -> Result<HubClient, String> {
        let base = normalize_code(base);
        let body = serde_json::json!({ "name": name, "pin": pin }).to_string();
        let (status, text) = request(&base, "POST", "/api/signup", None, Some(&body))?;
        parse_auth(status, &text, base)
    }

    pub fn login(base: &str, name: &str, pin: &str) -> Result<HubClient, String> {
        let base = normalize_code(base);
        let body = serde_json::json!({ "name": name, "pin": pin }).to_string();
        let (status, text) = request(&base, "POST", "/api/login", None, Some(&body))?;
        parse_auth(status, &text, base)
    }

    /// Sign up, falling back to sign in when the name already exists.
    pub fn connect(base: &str, name: &str, pin: &str) -> Result<HubClient, String> {
        match HubClient::signup(base, name, pin) {
            Ok(c) => Ok(c),
            Err(e) => {
                if e.contains("taken") {
                    HubClient::login(base, name, pin)
                } else {
                    Err(e)
                }
            }
        }
    }

    pub fn snapshot(&self) -> Result<Snapshot, String> {
        let (status, text) = request(&self.base, "GET", "/api/state", Some(&self.token), None)?;
        if status != 200 {
            return Err(error_text(&text, "Could not reach the space."));
        }
        serde_json::from_str(&text).map_err(|_| "The space sent something unexpected.".to_owned())
    }

    pub fn send(&self, conv: u64, text: &str, reply_to: Option<u64>) -> Result<(), String> {
        let body = serde_json::json!({ "conv": conv, "text": text, "reply_to": reply_to }).to_string();
        let (status, text) = request(&self.base, "POST", "/api/message", Some(&self.token), Some(&body))?;
        if status != 200 {
            return Err(error_text(&text, "The message did not send."));
        }
        Ok(())
    }

    pub fn read(&self, conv: u64) -> Result<(), String> {
        let body = serde_json::json!({ "conv": conv }).to_string();
        let (status, text) = request(&self.base, "POST", "/api/read", Some(&self.token), Some(&body))?;
        if status != 200 {
            return Err(error_text(&text, "Could not mark the chat read."));
        }
        Ok(())
    }

    pub fn typing(&self, conv: u64) -> Result<(), String> {
        let body = serde_json::json!({ "conv": conv }).to_string();
        let _ = request(&self.base, "POST", "/api/typing", Some(&self.token), Some(&body))?;
        Ok(())
    }

    pub fn create_conv(&self, members: &[String], title: &str) -> Result<u64, String> {
        let body = serde_json::json!({ "members": members, "title": title }).to_string();
        let (status, text) = request(
            &self.base,
            "POST",
            "/api/conversation",
            Some(&self.token),
            Some(&body),
        )?;
        if status != 200 {
            return Err(error_text(&text, "Could not start that chat."));
        }
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|_| "The space sent something unexpected.".to_owned())?;
        value
            .get("id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| "The space sent something unexpected.".to_owned())
    }

    pub fn react(&self, message: u64, emoji: &str) -> Result<(), String> {
        let body = serde_json::json!({ "message": message, "emoji": emoji }).to_string();
        let (status, text) = request(&self.base, "POST", "/api/react", Some(&self.token), Some(&body))?;
        if status != 200 {
            return Err(error_text(&text, "Could not add that reaction."));
        }
        Ok(())
    }
}

fn parse_auth(status: u16, text: &str, base: String) -> Result<HubClient, String> {
    if status != 200 {
        return Err(error_text(text, "Could not connect to that space."));
    }
    let auth: AuthReply = serde_json::from_str(text)
        .map_err(|_| "The space sent something unexpected.".to_owned())?;
    Ok(HubClient {
        base,
        token: auth.token,
        me: auth.me,
    })
}

fn error_text(text: &str, fallback: &str) -> String {
    serde_json::from_str::<ErrorReply>(text)
        .ok()
        .and_then(|e| e.error)
        .filter(|e| !e.trim().is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

struct Endpoint {
    secure: bool,
    /// Host without IPv6 brackets, as DNS and TLS want it.
    host: String,
    port: u16,
    /// What goes in the Host header, brackets and port included when needed.
    authority: String,
}

fn endpoint_of(base: &str) -> Result<Endpoint, String> {
    let norm = normalize_code(base);
    let (secure, rest) = if let Some(r) = norm.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = norm.strip_prefix("http://") {
        (false, r)
    } else {
        return Err("That space address doesn't look right.".to_owned());
    };
    if rest.is_empty() {
        return Err("That space address doesn't look right.".to_owned());
    }
    let default_port = if secure { 443 } else { 80 };
    let (host, port) = split_host_port(rest, default_port)?;
    let shown = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.clone()
    };
    let authority = if port == default_port {
        shown
    } else {
        format!("{shown}:{port}")
    };
    Ok(Endpoint {
        secure,
        host,
        port,
        authority,
    })
}

fn split_host_port(authority: &str, default_port: u16) -> Result<(String, u16), String> {
    if let Some(stripped) = authority.strip_prefix('[') {
        let Some((inside, after)) = stripped.split_once(']') else {
            return Err("That space address doesn't look right.".to_owned());
        };
        let port = match after.strip_prefix(':') {
            Some(p) => p
                .parse::<u16>()
                .map_err(|_| "That space address doesn't look right.".to_owned())?,
            None => default_port,
        };
        return Ok((inside.to_owned(), port));
    }
    match authority.rsplit_once(':') {
        Some((host, p)) if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) => {
            let port = p
                .parse::<u16>()
                .map_err(|_| "That space address doesn't look right.".to_owned())?;
            Ok((host.to_owned(), port))
        }
        _ => Ok((authority.to_owned(), default_port)),
    }
}

fn request(
    base: &str,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<&str>,
) -> Result<(u16, String), String> {
    let ep = endpoint_of(base)?;
    let addr = (ep.host.as_str(), ep.port)
        .to_socket_addrs()
        .map_err(|_| "That space address doesn't look right.".to_owned())?
        .next()
        .ok_or_else(|| "That space address doesn't look right.".to_owned())?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(3)).map_err(|_| {
        "Could not reach that space. Check the address and that the hub is running.".to_owned()
    })?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    let body = body.unwrap_or("");
    let mut req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nAccept: application/json\r\n",
        ep.authority
    );
    if let Some(t) = token {
        req.push_str(&format!("Authorization: Bearer {t}\r\n"));
    }
    if !body.is_empty() {
        req.push_str("Content-Type: application/json\r\n");
        req.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    req.push_str("\r\n");
    req.push_str(body);

    let raw = if ep.secure {
        tls_exchange(stream, req.as_bytes(), &ep.host)?
    } else {
        exchange(&mut stream, req.as_bytes())
            .map_err(|_| "The connection dropped while sending.".to_owned())?
    };

    let text = String::from_utf8_lossy(&raw).to_string();
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| "That space didn't answer.".to_owned())?;
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    Ok((status, body.to_owned()))
}

fn exchange<S: Read + Write>(stream: &mut S, req: &[u8]) -> std::io::Result<Vec<u8>> {
    stream.write_all(req)?;
    stream.flush()?;
    let mut raw = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => raw.extend_from_slice(&chunk[..n]),
            Err(e) => {
                // Some servers close without a TLS goodbye. Keep a complete
                // answer; report an error when nothing arrived.
                if raw.is_empty() {
                    return Err(e);
                }
                break;
            }
        }
    }
    Ok(raw)
}

fn tls_exchange(stream: TcpStream, req: &[u8], host: &str) -> Result<Vec<u8>, String> {
    static INSTALL_PROVIDER: Once = Once::new();
    INSTALL_PROVIDER.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });

    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(host.to_owned())
        .map_err(|_| "That space address doesn't look right.".to_owned())?;
    let conn = rustls::ClientConnection::new(Arc::new(config), name)
        .map_err(|_| "Could not set up the secure link.".to_owned())?;
    let mut tls = rustls::StreamOwned::new(conn, stream);
    exchange(&mut tls, req).map_err(|e| {
        format!("The secure link to that space failed ({e}). Check its certificate or use http:// on your own network.")
    })
}

// ---------------------------------------------------------------- web assets

fn web_index() -> &'static str {
    include_str!("../../web/index.html")
}
fn web_css() -> &'static str {
    include_str!("../../web/mobile.css")
}
fn web_js() -> &'static str {
    include_str!("../../web/mobile.js")
}
fn web_manifest() -> &'static str {
    include_str!("../../web/manifest.webmanifest")
}
fn web_sw() -> &'static str {
    include_str!("../../web/sw.js")
}
fn web_icon() -> &'static str {
    include_str!("../../web/icon.svg")
}

/// Builds a ready-to-scan hub URL line for the desktop link screen.
pub fn hub_url(port: u16) -> String {
    format!("http://{}", share_host(port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_round_trip_and_old_records_still_work() {
        let rec = pin_record("Ava", "1234");
        assert!(rec.starts_with("s2$"));
        assert!(pin_matches(&rec, "Ava", "1234"));
        assert!(pin_matches(&rec, "ava", "1234"));
        assert!(!pin_matches(&rec, "Ava", "12345"));
        assert!(!pin_matches("nonsense", "Ava", "1234"));
        let legacy = legacy_digest("Ava", "1234");
        assert!(pin_matches(&legacy, "ava", "1234"));
        assert!(!pin_matches(&legacy, "Ava", "0000"));
    }

    #[test]
    fn addresses_keep_https_and_default_ports() {
        assert_eq!(
            normalize_code("chat.example.com"),
            "http://chat.example.com:8777"
        );
        assert_eq!(
            normalize_code("https://chat.example.com/"),
            "https://chat.example.com"
        );
        assert_eq!(
            normalize_code("http://192.168.1.5:9000"),
            "http://192.168.1.5:9000"
        );
        assert_eq!(
            normalize_code("https://chat.example.com:8443"),
            "https://chat.example.com:8443"
        );
        let ep = endpoint_of("https://chat.example.com").unwrap();
        assert!(ep.secure);
        assert_eq!(ep.port, 443);
        assert_eq!(ep.authority, "chat.example.com");
        let ep = endpoint_of("192.168.1.5:8777").unwrap();
        assert!(!ep.secure);
        assert_eq!(ep.port, 8777);
        assert_eq!(ep.authority, "192.168.1.5:8777");
        assert_eq!(endpoint_of("[::1]:9000").unwrap().host, "::1");
    }

    #[test]
    #[ignore = "needs internet: proves the https client path completes a real handshake"]
    fn https_request_completes() {
        let (status, body) = request("https://example.com", "GET", "/", None, None).unwrap();
        assert_eq!(status, 200);
        assert!(body.to_lowercase().contains("example"));
    }

    #[test]
    fn a_hub_and_its_clients_talk_end_to_end() {
        let dir = std::env::temp_dir().join(format!("feather-test-{}", now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let hub = Hub::start_with(HubConfig {
            bind: "127.0.0.1".to_owned(),
            port: 0,
            data_path: dir.join("hub.json"),
            strict_port: true,
            signup_open: true,
        })
        .unwrap();
        let base = format!("127.0.0.1:{}", hub.port);

        let ava = HubClient::signup(&base, "Ava", "1234").unwrap();
        let ben = HubClient::signup(&base, "Ben", "5678").unwrap();
        let snap = ava.snapshot().unwrap();
        let ben_id = snap
            .people
            .iter()
            .find(|p| p.name == "Ben")
            .expect("Ben is listed")
            .id
            .clone();
        let conv = ava.create_conv(&[ben_id], "").unwrap();
        ava.send(conv, "Hello over the hub", None).unwrap();

        let seen = ben.snapshot().unwrap();
        assert_eq!(seen.convs[0].messages.last().unwrap().text, "Hello over the hub");
        assert_eq!(seen.convs[0].unread, 1);

        ben.read(conv).unwrap();
        let seen_again = ben.snapshot().unwrap();
        assert_eq!(seen_again.convs[0].unread, 0);

        let again = HubClient::login(&base, "Ava", "1234").unwrap();
        assert_eq!(again.me.name, "Ava");

        drop(hub);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

