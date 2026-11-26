const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

async fn fetch_character_html(first: &str, last: &str) -> Result<(u16, String), reqwest::Error> {
    let url = format!(
        "https://worldofwarcraft.blizzard.com/en-us/forever/us/armory/character/{}-{}",
        first.to_lowercase(),
        last.to_lowercase()
    );
    let resp = reqwest::Client::builder().user_agent(UA).build()?.get(&url).send().await?;
    let status = resp.status().as_u16();
    Ok((status, resp.text().await?))
}

use serde::Deserialize;

/// Values parsed out of the armory page.
#[derive(Debug)]
pub struct Profile {
    pub name: String,
    pub level: u32,
    pub item_level: u32,
}

/// Fetches the armory page for a character and parses it.
pub async fn fetch_profile(first: &str, last: &str) -> Result<Profile, Box<dyn std::error::Error>> {
    let (_status, html) = fetch_character_html(first, last).await?;
    Ok(parse_character(&html)?)
}

#[derive(Debug)]
pub enum ParseError {
    /// Both sources failed; carries the reason for each.
    NoSource { json: String, meta: String },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ParseError::NoSource { json, meta } = self;
        write!(f, "could not parse character (embedded JSON: {json}; meta fallback: {meta})")
    }
}

impl std::error::Error for ParseError {}

/// Primary source: the `characterProfileInitialState` JSON embedded in the page.
/// Only the fields we need are declared, so extra fields added by Blizzard are ignored.
#[derive(Deserialize)]
struct InitialState {
    character: JsonCharacter,
}

#[derive(Deserialize)]
struct JsonCharacter {
    name: String,
    surname: Option<String>,
    level: u32,
    #[serde(rename = "averageItemLevel")]
    average_item_level: u32,
}

/// Text between `start` and `end` markers.
fn between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let i = s.find(start)? + start.len();
    let j = s[i..].find(end)? + i;
    Some(&s[i..j])
}

fn parse_json(html: &str) -> Result<Profile, String> {
    const MARKER: &str = "characterProfileInitialState = ";
    let start = html.find(MARKER).ok_or("marker not found")? + MARKER.len();
    // Parse just the first JSON value; trailing `;</script>...` is ignored.
    let state: InitialState = serde_json::Deserializer::from_str(&html[start..])
        .into_iter::<InitialState>()
        .next()
        .ok_or("empty payload")?
        .map_err(|e| e.to_string())?;
    let c = state.character;
    let name = match c.surname.filter(|s| !s.is_empty()) {
        Some(surname) => format!("{} {}", c.name, surname),
        None => c.name,
    };
    Ok(Profile { name, level: c.level, item_level: c.average_item_level })
}

/// Fallback: `<title>Name - Character</title>` and
/// `<meta name="description" content="Name ❮Guild❯ - 26 Gnome ..., 17 ilvl"/>`.
fn parse_meta(html: &str) -> Option<Profile> {
    let name = between(html, "<title>", " - Character")?.trim().to_string();
    let desc = between(html, r#"<meta name="description" content=""#, r#"""#)?;
    let tail = desc.split(" - ").last()?; // "26 Gnome character.class, 17 ilvl"
    let level = tail.split_whitespace().next()?.parse().ok()?;
    let item_level = tail.rsplit(", ").next()?.strip_suffix(" ilvl")?.parse().ok()?;
    Some(Profile { name, level, item_level })
}

/// Tries the embedded JSON first, then falls back to the meta tags.
fn parse_character(html: &str) -> Result<Profile, ParseError> {
    match parse_json(html) {
        Ok(c) => Ok(c),
        Err(json) => parse_meta(html).ok_or(ParseError::NoSource {
            json,
            meta: "title/description not found or unexpected format".into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const META: &str = r#"<title>Foo Bar - Character</title><meta name="description" content="Foo ❮G❯ - 26 Gnome character.class, 17 ilvl"/>"#;

    #[test]
    fn json_preferred() {
        let html = format!(
            r#"{META}<script>var characterProfileInitialState = {{"character":{{"name":"Foo","surname":"Bar","level":30,"averageItemLevel":99,"extra":{{"x":1}}}},"summary":{{}}}};
</script>"#
        );
        let c = parse_character(&html).unwrap();
        assert_eq!((c.name.as_str(), c.level, c.item_level), ("Foo Bar", 30, 99));
    }

    #[test]
    fn falls_back_to_meta_when_json_missing_or_reshaped() {
        for html in [
            META.to_string(),
            format!(r#"{META}var characterProfileInitialState = {{"character":{{"renamed":1}}}};"#),
        ] {
            let c = parse_character(&html).unwrap();
            assert_eq!((c.name.as_str(), c.level, c.item_level), ("Foo Bar", 26, 17));
        }
    }

    #[test]
    fn errors_when_both_fail() {
        assert!(parse_character("<html></html>").is_err());
    }
}
