//! Microsoft account sign-in and Minecraft license verification.
//!
//! Flow: the system browser opens Microsoft's sign-in page (OAuth 2.0
//! authorization code + PKCE). Microsoft redirects to a one-shot HTTP listener
//! on 127.0.0.1, which hands us the code. The code is exchanged for Microsoft
//! tokens, then Xbox Live -> XSTS -> Minecraft services, and finally the
//! account's entitlements and profile are checked to confirm it owns
//! Minecraft: Java Edition. Only the resulting session is stored, encrypted.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result, anyhow, bail};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::net::{self, url_decode, url_encode};
use crate::{paths, secure_store};

const AUTHORIZE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBL_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_ENTITLEMENTS_URL: &str = "https://api.minecraftservices.com/entitlements/mcstore";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const SCOPE: &str = "XboxLive.signin offline_access";
const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    pub refresh_token: String,
    pub access_token: String,
    /// Unix seconds when `access_token` stops working.
    pub expires_at: u64,
    pub uuid: String,
    pub name: String,
    pub skin_url: Option<String>,
}

impl std::fmt::Debug for Session {
    // Never print tokens.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").field("name", &self.name).field("uuid", &self.uuid).finish_non_exhaustive()
    }
}

impl Session {
    /// True when the Minecraft token expires within ten minutes.
    pub fn needs_refresh(&self) -> bool {
        now() + 600 >= self.expires_at
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Returned when the saved session can no longer be used and the user has to
/// sign in again (revoked or expired refresh token, lost license, ...).
#[derive(Debug)]
pub struct SignInRequired(pub String);

impl std::fmt::Display for SignInRequired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SignInRequired {}

pub fn requires_sign_in(err: &anyhow::Error) -> bool {
    err.downcast_ref::<SignInRequired>().is_some()
}

// ---------------------------------------------------------------------------
// Persistence

pub fn load_saved() -> Option<Session> {
    let bytes = secure_store::load(&paths::account_file()).ok()??;
    serde_json::from_slice(&bytes).ok()
}

pub fn save(session: &Session) -> Result<()> {
    secure_store::save(&paths::account_file(), &serde_json::to_vec(session)?)
}

pub fn forget() {
    secure_store::delete(&paths::account_file());
}

// ---------------------------------------------------------------------------
// Browser sign-in

fn random_token(bytes: usize) -> Result<String> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|e| anyhow!("random number generator failed: {e}"))?;
    Ok(URL_SAFE_NO_PAD.encode(buf))
}

/// Opens the browser and waits for the user to finish signing in.
/// `cancel` aborts the wait (the user closed the dialog).
pub fn sign_in(client_id: &str, cancel: &AtomicBool) -> Result<Session> {
    let verifier = random_token(48)?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = random_token(16)?;

    // Listen on both loopback stacks: browsers may resolve "localhost" to either.
    let v4 = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).context("starting sign-in listener")?;
    let port = v4.local_addr()?.port();
    let mut listeners = vec![v4];
    if let Ok(v6) = TcpListener::bind(SocketAddr::from((Ipv6Addr::LOCALHOST, port))) {
        listeners.push(v6);
    }
    for l in &listeners {
        l.set_nonblocking(true)?;
    }
    let redirect_uri = format!("http://localhost:{port}");

    let url = format!(
        "{AUTHORIZE_URL}?client_id={}&response_type=code&redirect_uri={}&response_mode=query\
         &scope={}&code_challenge={challenge}&code_challenge_method=S256&state={state}&prompt=select_account",
        url_encode(client_id),
        url_encode(&redirect_uri),
        url_encode(SCOPE),
    );
    open::that(&url).context("could not open your web browser")?;

    let code = wait_for_redirect(&listeners, &state, cancel)?;
    drop(listeners);

    let tokens = ms_token(&[
        ("client_id", client_id),
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri", &redirect_uri),
        ("code_verifier", &verifier),
        ("scope", SCOPE),
    ])?;
    minecraft_session(tokens)
}

/// Uses the stored refresh token to get a fresh Minecraft token. Also
/// re-checks the license, so a refunded or expired Game Pass account stops
/// working.
pub fn refresh(session: &Session, client_id: &str) -> Result<Session> {
    let tokens = ms_token(&[
        ("client_id", client_id),
        ("grant_type", "refresh_token"),
        ("refresh_token", &session.refresh_token),
        ("scope", SCOPE),
    ])?;
    minecraft_session(tokens)
}

struct Callback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

fn wait_for_redirect(listeners: &[TcpListener], state: &str, cancel: &AtomicBool) -> Result<String> {
    let deadline = Instant::now() + SIGN_IN_TIMEOUT;
    loop {
        if cancel.load(Ordering::Relaxed) {
            bail!(net::Cancelled);
        }
        if Instant::now() > deadline {
            bail!("Sign-in timed out. Please try again.");
        }
        for l in listeners {
            let Ok((stream, _)) = l.accept() else { continue };
            let Some(cb) = read_callback(stream) else { continue };
            if let Some(err) = cb.error {
                bail!("Microsoft sign-in was not completed ({err}).");
            }
            if cb.state.as_deref() != Some(state) {
                bail!("Sign-in response did not match this request. Please try again.");
            }
            if let Some(code) = cb.code {
                return Ok(code);
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Reads one HTTP request. Returns `None` for unrelated requests such as
/// `/favicon.ico`, which get a 404 so the browser stops asking.
fn read_callback(mut stream: TcpStream) -> Option<Callback> {
    stream.set_nonblocking(false).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut buf = Vec::with_capacity(2048);
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < 16 * 1024 {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    let request = String::from_utf8_lossy(&buf);
    let target = request.lines().next()?.split_whitespace().nth(1)?.to_owned();
    let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");

    let mut cb = Callback { code: None, state: None, error: None };
    let mut description = None;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let v = url_decode(v);
        match k {
            "code" => cb.code = Some(v),
            "state" => cb.state = Some(v),
            "error" => cb.error = Some(v),
            "error_description" => description = Some(v),
            _ => {}
        }
    }
    if cb.code.is_none() && cb.error.is_none() {
        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        return None;
    }
    let (title, body) = if cb.code.is_some() {
        ("You're signed in", "You can close this tab and return to Exile.".to_owned())
    } else {
        ("Sign-in was not completed", description.unwrap_or_else(|| "You can close this tab and try again.".into()))
    };
    let html = callback_page(title, &body);
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n{html}",
        html.len()
    );
    let _ = stream.flush();
    Some(cb)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn callback_page(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=utf-8><title>Exile</title><style>\
         body{{margin:0;height:100vh;display:flex;align-items:center;justify-content:center;\
         background:#111419;color:#e6eaf0;font:15px 'Segoe UI',system-ui,sans-serif}}\
         div{{text-align:center}}h1{{font-weight:600;font-size:20px;margin:0 0 8px}}\
         p{{margin:0;color:#97a2b3}}b{{display:block;letter-spacing:.3em;font-size:12px;color:#678fc5;margin-bottom:24px}}\
         </style></head><body><div><b>EXILE</b><h1>{}</h1><p>{}</p></div></body></html>",
        html_escape(title),
        html_escape(body)
    )
}

// ---------------------------------------------------------------------------
// Token chain

struct MsTokens {
    access_token: String,
    refresh_token: String,
}

fn post_json(url: &str, body: &Value, bearer: Option<&str>) -> Result<(u16, Value)> {
    let mut req = net::agent().post(url).header("Accept", "application/json");
    if let Some(token) = bearer {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    let mut resp = req.send_json(body).with_context(|| format!("could not reach {}", host(url)))?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

fn get_json_auth(url: &str, bearer: &str) -> Result<(u16, Value)> {
    let mut resp = net::agent()
        .get(url)
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {bearer}"))
        .call()
        .with_context(|| format!("could not reach {}", host(url)))?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

fn host(url: &str) -> &str {
    url.split("://").nth(1).and_then(|r| r.split('/').next()).unwrap_or(url)
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key)?.as_str().map(str::to_owned)
}

fn ms_token(form: &[(&str, &str)]) -> Result<MsTokens> {
    let mut resp = net::agent()
        .post(TOKEN_URL)
        .header("Accept", "application/json")
        .send_form(form.iter().copied())
        .context("could not reach Microsoft")?;
    let status = resp.status().as_u16();
    let v: Value = serde_json::from_str(&resp.body_mut().read_to_string().unwrap_or_default()).unwrap_or(Value::Null);
    if status != 200 {
        let code = str_field(&v, "error").unwrap_or_default();
        if code == "invalid_grant" {
            bail!(SignInRequired("Your Microsoft session has expired. Please sign in again.".into()));
        }
        if code == "invalid_client" || code == "unauthorized_client" {
            bail!("Microsoft rejected the launcher's app registration ({code}). Check the client ID.");
        }
        bail!("Microsoft sign-in failed (HTTP {status} {code}).");
    }
    Ok(MsTokens {
        access_token: str_field(&v, "access_token").ok_or_else(|| anyhow!("Microsoft returned no access token"))?,
        refresh_token: str_field(&v, "refresh_token").ok_or_else(|| anyhow!("Microsoft returned no refresh token"))?,
    })
}

fn xsts_error(xerr: u64) -> String {
    match xerr {
        2148916233 => "This Microsoft account has no Xbox profile yet. Sign in once at minecraft.net to create one, then try again.".into(),
        2148916235 => "Xbox Live is not available in your country or region.".into(),
        2148916236 | 2148916237 => "This account needs adult verification on the Xbox website before it can play.".into(),
        2148916238 => "This is a child account. An adult needs to add it to a Microsoft family group first.".into(),
        other => format!("Xbox Live sign-in failed (error {other})."),
    }
}

fn minecraft_session(ms: MsTokens) -> Result<Session> {
    // Xbox Live user token
    let (status, xbl) = post_json(
        XBL_URL,
        &json!({
            "Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={}", ms.access_token)},
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }),
        None,
    )?;
    if status != 200 {
        bail!("Xbox Live sign-in failed (HTTP {status}).");
    }
    let xbl_token = str_field(&xbl, "Token").ok_or_else(|| anyhow!("Xbox Live returned no token"))?;

    // XSTS token for Minecraft services
    let (status, xsts) = post_json(
        XSTS_URL,
        &json!({
            "Properties": {"SandboxId": "RETAIL", "UserTokens": [xbl_token]},
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }),
        None,
    )?;
    if status == 401 {
        let xerr = xsts.get("XErr").and_then(Value::as_u64).unwrap_or(0);
        bail!(SignInRequired(xsts_error(xerr)));
    }
    if status != 200 {
        bail!("Xbox Live authorization failed (HTTP {status}).");
    }
    let xsts_token = str_field(&xsts, "Token").ok_or_else(|| anyhow!("Xbox Live returned no XSTS token"))?;
    let uhs = xsts
        .pointer("/DisplayClaims/xui/0/uhs")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Xbox Live returned no user hash"))?;

    // Minecraft access token
    let (status, mc) = post_json(MC_LOGIN_URL, &json!({"identityToken": format!("XBL3.0 x={uhs};{xsts_token}")}), None)?;
    if status == 403 {
        bail!(
            "Minecraft services rejected this launcher. The app registration has not been approved \
             for Minecraft yet (see https://aka.ms/mce-reviewappid)."
        );
    }
    if status == 429 {
        bail!("Too many sign-in attempts. Wait a minute and try again.");
    }
    if status != 200 {
        bail!("Minecraft sign-in failed (HTTP {status}).");
    }
    let access_token = str_field(&mc, "access_token").ok_or_else(|| anyhow!("Minecraft returned no access token"))?;
    let expires_in = mc.get("expires_in").and_then(Value::as_u64).unwrap_or(86_400);

    // License: entitlements first, then the profile (which only exists for
    // accounts that can play, including Game Pass).
    let (status, ent) = get_json_auth(MC_ENTITLEMENTS_URL, &access_token)?;
    let owns_game = status == 200
        && ent.get("items").and_then(Value::as_array).is_some_and(|items| {
            items
                .iter()
                .filter_map(|i| i.get("name").and_then(Value::as_str))
                .any(|n| n == "game_minecraft" || n == "product_minecraft")
        });

    let (status, profile) = get_json_auth(MC_PROFILE_URL, &access_token)?;
    if status == 404 {
        let msg = if owns_game {
            "Your Minecraft profile isn't set up yet. Choose a username at minecraft.net, then sign in again."
        } else {
            "This Microsoft account doesn't own Minecraft: Java Edition."
        };
        bail!(SignInRequired(msg.into()));
    }
    if status != 200 {
        bail!("Could not load your Minecraft profile (HTTP {status}).");
    }
    let uuid = str_field(&profile, "id").ok_or_else(|| anyhow!("profile has no id"))?;
    let name = str_field(&profile, "name").ok_or_else(|| anyhow!("profile has no name"))?;
    let skin_url = profile
        .get("skins")
        .and_then(Value::as_array)
        .and_then(|skins| {
            skins
                .iter()
                .find(|s| s.get("state").and_then(Value::as_str) == Some("ACTIVE"))
                .or_else(|| skins.first())
        })
        .and_then(|s| str_field(s, "url"))
        .map(|u| u.replacen("http://", "https://", 1));

    Ok(Session {
        refresh_token: ms.refresh_token,
        access_token,
        expires_at: now() + expires_in,
        uuid,
        name,
        skin_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_parsing() {
        let (a, b) = loopback_pair();
        let mut client = a;
        client.write_all(b"GET /?code=M.C5%2Babc&state=xyz HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
        let cb = read_callback(b).unwrap();
        assert_eq!(cb.code.as_deref(), Some("M.C5+abc"));
        assert_eq!(cb.state.as_deref(), Some("xyz"));
    }

    #[test]
    fn favicon_ignored() {
        let (mut client, server) = loopback_pair();
        client.write_all(b"GET /favicon.ico HTTP/1.1\r\n\r\n").unwrap();
        assert!(read_callback(server).is_none());
    }

    fn loopback_pair() -> (TcpStream, TcpStream) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(l.local_addr().unwrap()).unwrap();
        let (server, _) = l.accept().unwrap();
        (client, server)
    }

    /// UI preview helper: stores a placeholder session under EXILE_HOME so the
    /// signed-in layout can be checked without a Microsoft account.
    #[test]
    #[ignore]
    fn preview_session() {
        assert!(std::env::var_os("EXILE_HOME").is_some(), "refusing to touch the real profile");
        save(&Session {
            refresh_token: String::new(),
            access_token: String::new(),
            expires_at: now() + 3600,
            uuid: "00000000000000000000000000000000".into(),
            name: "Steve".into(),
            skin_url: None,
        })
        .unwrap();
    }

    #[test]
    fn escapes_html() {
        assert_eq!(html_escape("<a&\">"), "&lt;a&amp;&quot;&gt;");
    }
}
