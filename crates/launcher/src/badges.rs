//! Exile badges, from the badge service on exiles.dev (the website repo has
//! the service; the art is shared with Exile Client).
//!
//! Anyone's shown badges can be looked up without signing in. Seeing all of
//! your own and choosing which show needs a badge session. Mojang refuses
//! requests from the service's host, so the service can't ask it who you are;
//! instead you prove it with what Mojang has already signed: your account's
//! chat key and your profile (UUID and name). Exile fetches both from Mojang,
//! signs the service's one-time id with the key, and the service checks the
//! signatures against Mojang's public keys. The Minecraft token and the private
//! key never leave this computer.

use std::collections::HashMap;

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::Session;
use crate::net;

const API: &str = "https://exiles.dev/api";
const MOJANG_SESSION: &str = "https://sessionserver.mojang.com";
const MINECRAFT_SERVICES: &str = "https://api.minecraftservices.com";

/// For testing: `EXILE_API_URL` points at another badge service, and
/// `EXILE_MOJANG_SESSION_URL` and `EXILE_MINECRAFT_SERVICES_URL` at stand-ins
/// for Mojang's.
fn from_env(var: &str, default: &str, path: &str) -> String {
    let base = std::env::var(var).ok().filter(|u| !u.trim().is_empty()).unwrap_or_else(|| default.to_owned());
    format!("{}{path}", base.trim_end_matches('/'))
}

fn api(path: &str) -> String {
    from_env("EXILE_API_URL", API, path)
}

pub struct Badge {
    pub id: &'static str,
    pub title: &'static str,
    pub about: &'static str,
    /// 16x16 PNG, the same file Exile Client draws.
    pub png: &'static [u8],
}

/// Every badge, in the order they're shown.
pub const ALL: [Badge; 4] = [
    Badge {
        id: "exile",
        title: "Exile",
        about: "Part of the team behind Exile",
        png: include_bytes!("../../../client/src/main/resources/assets/exile/textures/font/badge/exile.png"),
    },
    Badge {
        id: "dev",
        title: "Developer",
        about: "Helped build Exile",
        png: include_bytes!("../../../client/src/main/resources/assets/exile/textures/font/badge/dev.png"),
    },
    Badge {
        id: "ogu",
        title: "OG name",
        about: "A rare, original username",
        png: include_bytes!("../../../client/src/main/resources/assets/exile/textures/font/badge/ogu.png"),
    },
    Badge {
        id: "early_supporter",
        title: "Early supporter",
        about: "Used Exile before 11 April 2027",
        png: include_bytes!("../../../client/src/main/resources/assets/exile/textures/font/badge/early_supporter.png"),
    },
];

pub fn find(id: &str) -> Option<&'static Badge> {
    ALL.iter().find(|b| b.id == id)
}

/// Badge ids this launcher knows, in display order; unknown ones are dropped.
fn known(ids: &[String]) -> Vec<String> {
    ALL.iter().filter(|b| ids.iter().any(|i| i == b.id)).map(|b| b.id.to_owned()).collect()
}

/// Shown badges of these players (UUIDs without dashes). Players without any are left out.
pub fn lookup(uuids: &[String]) -> Result<HashMap<String, Vec<String>>> {
    #[derive(Deserialize)]
    struct Answer {
        badges: HashMap<String, Vec<String>>,
    }
    let answer: Answer = parse(post(&api("/badges"), &json!({ "uuids": uuids }))?)?;
    Ok(answer.badges.into_iter().map(|(uuid, ids)| (uuid, known(&ids))).collect())
}

/// Your own badges: every one you have, and the ones you've hidden.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mine {
    pub token: String,
    pub badges: Vec<String>,
    pub hidden: Vec<String>,
}

impl Mine {
    pub fn shown(&self) -> Vec<String> {
        self.badges.iter().filter(|b| !self.hidden.contains(b)).cloned().collect()
    }
}

#[derive(Deserialize)]
struct Own {
    #[serde(default)]
    token: Option<String>,
    badges: Vec<String>,
    hidden: Vec<String>,
}

/// Starts a badge session for this account (see the module docs).
pub fn sign_in(session: &Session) -> Result<Mine> {
    let start = post(&api("/session/start"), &json!({}))?;
    let server_id = start.get("serverId").and_then(Value::as_str).context("the badge service didn't start a session")?.to_owned();

    let profile = signed_profile(&session.uuid)?;
    let key = chat_key(&session.access_token)?;
    let body = json!({
        "serverId": server_id,
        "profile": { "value": profile.value, "signature": profile.signature },
        "key": { "publicKey": key.public_key, "expiresAt": key.expires_at, "mojangSignature": key.mojang_signature },
        "signature": key.sign(server_id.as_bytes())?,
    });
    let own: Own = parse(post(&api("/session/finish"), &body)?)?;
    Ok(Mine { token: own.token.unwrap_or_default(), badges: known(&own.badges), hidden: known(&own.hidden) })
}

/// The "textures" property of the account's own profile: Mojang-signed JSON
/// holding its UUID, current name and when it was signed.
struct SignedProfile {
    value: String,
    signature: String,
}

fn signed_profile(uuid: &str) -> Result<SignedProfile> {
    #[derive(Deserialize)]
    struct Profile {
        properties: Vec<Property>,
    }
    #[derive(Deserialize)]
    struct Property {
        name: String,
        value: String,
        #[serde(default)]
        signature: Option<String>,
    }
    let url = from_env("EXILE_MOJANG_SESSION_URL", MOJANG_SESSION, &format!("/session/minecraft/profile/{uuid}?unsigned=false"));
    let profile: Profile = net::get_json(&url).context("could not get your profile from Mojang")?;
    let textures = profile.properties.into_iter().find(|p| p.name == "textures").context("Mojang sent no profile signature")?;
    Ok(SignedProfile { value: textures.value, signature: textures.signature.context("Mojang sent no profile signature")? })
}

/// The account's chat key, as Mojang hands it out (what Minecraft signs chat with).
struct ChatKey {
    /// PKCS#8 DER. `RsaPrivateKey` wipes its copy when dropped; this one is
    /// overwritten in `Drop` below.
    private_der: Vec<u8>,
    /// X.509 DER, base64.
    public_key: String,
    expires_at: String,
    /// Mojang's signature over UUID, expiry and public key, base64.
    mojang_signature: String,
}

impl ChatKey {
    /// SHA256withRSA, as Minecraft signs chat; base64.
    fn sign(&self, message: &[u8]) -> Result<String> {
        use base64::Engine as _;
        use rsa::pkcs1v15::SigningKey;
        use rsa::pkcs8::DecodePrivateKey as _;
        use rsa::signature::{SignatureEncoding as _, Signer as _};
        let key = rsa::RsaPrivateKey::from_pkcs8_der(&self.private_der).context("reading your chat key")?;
        let signature = SigningKey::<rsa::sha2::Sha256>::new(key).sign(message);
        Ok(base64::engine::general_purpose::STANDARD.encode(signature.to_vec()))
    }
}

impl Drop for ChatKey {
    fn drop(&mut self) {
        self.private_der.iter_mut().for_each(|b| *b = 0);
    }
}

fn chat_key(access_token: &str) -> Result<ChatKey> {
    use base64::Engine as _;
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Certificates {
        key_pair: KeyPair,
        public_key_signature_v2: String,
        expires_at: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct KeyPair {
        private_key: String,
        public_key: String,
    }
    let url = from_env("EXILE_MINECRAFT_SERVICES_URL", MINECRAFT_SERVICES, "/player/certificates");
    let mut resp = net::agent()
        .post(&url)
        .header("Authorization", format!("Bearer {access_token}"))
        .send_empty()
        .context("could not reach Mojang")?;
    if !resp.status().is_success() {
        bail!("Mojang didn't give out a chat key ({})", resp.status().as_u16());
    }
    let certs: Certificates = serde_json::from_str(&resp.body_mut().read_to_string()?).context("reading Mojang's chat key")?;
    let private_der = base64::engine::general_purpose::STANDARD.decode(pem_body(&certs.key_pair.private_key)).context("reading your chat key")?;
    Ok(ChatKey {
        private_der,
        public_key: pem_body(&certs.key_pair.public_key),
        expires_at: certs.expires_at,
        mojang_signature: certs.public_key_signature_v2,
    })
}

/// The base64 inside a PEM block. Mojang's headers say "RSA PRIVATE KEY" and
/// "RSA PUBLIC KEY", but the contents are PKCS#8 and X.509, as Minecraft reads them.
fn pem_body(pem: &str) -> String {
    pem.lines().filter(|l| !l.starts_with("-----")).flat_map(|l| l.split_whitespace()).collect()
}

/// Saves which of your badges are hidden.
pub fn set_hidden(mine: &Mine, hidden: &[String]) -> Result<Mine> {
    let mut resp = net::agent()
        .put(api("/me"))
        .header("Authorization", format!("Bearer {}", mine.token))
        .send_json(json!({ "hidden": hidden }))
        .context("could not reach exiles.dev")?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    if status != 200 {
        bail!("exiles.dev answered {status}");
    }
    let own: Own = serde_json::from_str(&text).context("reading badges")?;
    Ok(Mine { token: mine.token.clone(), badges: known(&own.badges), hidden: known(&own.hidden) })
}

fn post(url: &str, body: &Value) -> Result<Value> {
    let mut resp = net::agent().post(url).header("Accept", "application/json").send_json(body).context("could not reach exiles.dev")?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    if status != 200 {
        let error = serde_json::from_str::<Value>(&text).ok().and_then(|v| v.get("error")?.as_str().map(str::to_owned));
        bail!("{}", error.unwrap_or_else(|| format!("exiles.dev answered {status}")));
    }
    serde_json::from_str(&text).context("reading the badge service's answer")
}

fn parse<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T> {
    serde_json::from_value(value).context("reading the badge service's answer")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_badges_are_dropped_and_order_is_fixed() {
        let ids = ["early_supporter", "future_badge", "exile"].map(String::from);
        assert_eq!(known(&ids), ["exile", "early_supporter"]);
    }

    #[test]
    fn shown_leaves_out_hidden() {
        let mine = Mine { token: String::new(), badges: vec!["exile".into(), "ogu".into()], hidden: vec!["ogu".into()] };
        assert_eq!(mine.shown(), ["exile"]);
    }

    #[test]
    fn pem_body_is_the_base64_inside() {
        let pem = "-----BEGIN RSA PUBLIC KEY-----\nMIIB\nIjAN \r\nBgkq\n-----END RSA PUBLIC KEY-----\n";
        assert_eq!(pem_body(pem), "MIIBIjANBgkq");
    }

    #[test]
    fn signs_with_a_pkcs8_key() {
        use rsa::pkcs1v15::{Signature, VerifyingKey};
        use base64::Engine as _;
        use rsa::signature::Verifier as _;
        use rsa::pkcs8::DecodePrivateKey as _;
        // A throwaway 1024-bit test key (PKCS#8), made with openssl genpkey.
        const TEST_KEY: &str = "MIICdwIBADANBgkqhkiG9w0BAQEFAASCAmEwggJdAgEAAoGBAKu5Odf2uW5X5nuJc4+lRsN/8MbYIBScf4hKgeLLnRJgAGOE7eoIfSsmvfQT/GHA+PxMpLZoQVlosxGkKyxuqYFE5wce8ACXxUQS7JqhYKrNnMogaYyTgOzi9ewNg+vqtQD02rSa4CvkYX8eLQuOzYu3QkmMSy5+xpDqGwqHqSzLAgMBAAECgYEAqzseT5eonTuvCn4rqg+u+SApMc2uZ38JuBg7Xa+Upw7tNE8HgTcHwLB39tq67aqjW8j88RoSF5pL/IpLtpl+EFLL8K5EgD1QG5rAcNXAlUqTZi/zV27FfBbgcsC7tdXStGhoM8ndR5TE0/x+6U4LIDZSMFqtEfPMdMPWDG03zwkCQQDb4XAaJes3q70z3Hxh0+kl5P2IkVWRbT/NKwz4RguyEo3n39UX9ZB1zHU94WI6pg+WXdQDsVzSIi2Fjaod1pB9AkEAx+6nydTVuMn21f1yBYz4xNFdgVlA4i11DoECf+5YjwD/4r5pJmPzn/Sx/scHpRgtsSZvDQAbw+T3ABUsWIW85wJAQi22/Ne+1PGmr45EDXmnaZVjTZBYjb2w/0Je0/bZHE6DUW/IBj8NrLbN7sMAOqCyflJdNoNSaZiJ1PGPaw2iTQJAHqvoJbh3CSBCs9Vr1pon19nepGdS4t/iY1U7UvD3uQiIOPrJGtCeoV1iqRlTfOpWqCSjSLVJjSXlFqgo43Ow5QJBAKPHPr8ROkPCPEbdmnccyHc1DSVpGQ/vWaiFZzq87Ug5feX7FES9CNsmPKPy8YdoHu+gOjIWUpnonvYrhMGzg6A=";
        let der = base64::engine::general_purpose::STANDARD.decode(TEST_KEY).unwrap();
        let public = rsa::RsaPublicKey::from(&rsa::RsaPrivateKey::from_pkcs8_der(&der).unwrap());
        let key = ChatKey {
            private_der: der,
            public_key: String::new(),
            expires_at: String::new(),
            mojang_signature: String::new(),
        };
        let signed = base64::engine::general_purpose::STANDARD.decode(key.sign(b"server id").unwrap()).unwrap();
        let signature = Signature::try_from(signed.as_slice()).unwrap();
        assert!(VerifyingKey::<rsa::sha2::Sha256>::new(public).verify(b"server id", &signature).is_ok());
    }

    #[test]
    fn every_badge_has_its_art() {
        for b in &ALL {
            let img = image::load_from_memory(b.png).unwrap_or_else(|e| panic!("{}: {e}", b.id));
            assert_eq!((img.width(), img.height()), (16, 16), "{}", b.id);
        }
    }
}
