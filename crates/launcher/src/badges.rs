//! Exile badges, from the badge service on exiles.dev (the website repo has
//! the service; the art is shared with Exile Client).
//!
//! Anyone's shown badges can be looked up without signing in. Seeing all of
//! your own and choosing which show needs a badge session, which the service
//! gives out once Mojang confirms who you are, the same check Minecraft servers
//! use: Exile tells Mojang this account "joined" a server id from the service,
//! and the service asks Mojang. The Minecraft token only ever goes to Mojang.

use std::collections::HashMap;

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::Session;
use crate::net;

const API: &str = "https://exiles.dev/api";
const MOJANG_SESSION: &str = "https://sessionserver.mojang.com";

/// For testing: `EXILE_API_URL` points at another badge service, and
/// `EXILE_MOJANG_SESSION_URL` at a stand-in for Mojang's session server.
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
        about: "Played with Exile Client before 11 April 2027",
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

    let body = json!({ "accessToken": session.access_token, "selectedProfile": session.uuid, "serverId": server_id });
    let join = from_env("EXILE_MOJANG_SESSION_URL", MOJANG_SESSION, "/session/minecraft/join");
    let resp = net::agent().post(&join).send_json(&body).context("could not reach Mojang")?;
    if !resp.status().is_success() {
        bail!("Mojang didn't accept the sign-in ({})", resp.status().as_u16());
    }

    let own: Own = parse(post(&api("/session/finish"), &json!({ "name": session.name, "serverId": server_id, "client": false }))?)?;
    Ok(Mine { token: own.token.unwrap_or_default(), badges: known(&own.badges), hidden: known(&own.hidden) })
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
    fn every_badge_has_its_art() {
        for b in &ALL {
            let img = image::load_from_memory(b.png).unwrap_or_else(|e| panic!("{}: {e}", b.id));
            assert_eq!((img.width(), img.height()), (16, 16), "{}", b.id);
        }
    }
}
