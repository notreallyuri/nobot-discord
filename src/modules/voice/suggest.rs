use std::time::Duration;

const ENDPOINT: &str = "https://suggestqueries.google.com/complete/search";
const TIMEOUT: Duration = Duration::from_millis(1_200);

pub const MAX_CHOICES: usize = 25;
pub const MAX_CHOICE_LEN: usize = 100;

pub fn clip(text: &str) -> String {
    let text = text.trim();

    if text.chars().count() <= MAX_CHOICE_LEN {
        return text.to_string();
    }

    let kept: String = text.chars().take(MAX_CHOICE_LEN - 1).collect();
    format!("{kept}…")
}

fn parse(body: &serde_json::Value) -> Vec<String> {
    let Some(items) = body.get(1).and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(clip)
        .filter(|suggestion| !suggestion.is_empty())
        .take(MAX_CHOICES)
        .collect()
}

pub async fn youtube(http: &reqwest::Client, partial: &str) -> Vec<String> {
    let response = http
        .get(ENDPOINT)
        .query(&[
            ("client", "firefox"),
            ("ds", "yt"),
            ("oe", "utf-8"),
            ("q", partial),
        ])
        .timeout(TIMEOUT)
        .send()
        .await;

    let body = match response {
        Ok(response) if response.status().is_success() => response.json().await,
        Ok(response) => {
            tracing::debug!(status = %response.status(), "suggestion lookup rejected");
            return Vec::new();
        }
        Err(e) => {
            tracing::debug!(?e, "suggestion lookup failed");
            return Vec::new();
        }
    };

    match body {
        Ok(body) => parse(&body),
        Err(e) => {
            tracing::debug!(?e, "suggestion response was not the shape we expect");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(raw: &str) -> serde_json::Value {
        serde_json::from_str(raw).expect("valid json")
    }

    #[test]
    fn reads_the_suggestion_list() {
        let body = json(
            r#"["daft punk",["daft punk","daft punk get lucky","daft punk one more time"],[],{}]"#,
        );

        assert_eq!(
            parse(&body),
            [
                "daft punk",
                "daft punk get lucky",
                "daft punk one more time"
            ]
        );
    }

    #[test]
    fn survives_every_malformed_shape() {
        for raw in [
            "[]",
            r#"["only the echo"]"#,
            r#"["q",null,[],{}]"#,
            r#"["q","not an array",[],{}]"#,
            r#"["q",[1,2,3],[],{}]"#,
            "{}",
            "null",
        ] {
            assert!(parse(&json(raw)).is_empty(), "should have ignored: {raw}");
        }
    }

    #[test]
    fn drops_non_string_entries_but_keeps_the_rest() {
        let body = json(r#"["q",["good",42,null,"also good"],[],{}]"#);
        assert_eq!(parse(&body), ["good", "also good"]);
    }

    #[test]
    fn never_offers_more_than_discord_accepts() {
        let many: Vec<String> = (0..60).map(|i| format!(r#""track {i}""#)).collect();
        let body = json(&format!(r#"["q",[{}],[],{{}}]"#, many.join(",")));

        assert_eq!(parse(&body).len(), MAX_CHOICES);
    }

    #[test]
    fn clips_long_choices_by_character_not_byte() {
        let long = "a".repeat(200);
        let clipped = clip(&long);
        assert_eq!(clipped.chars().count(), MAX_CHOICE_LEN);
        assert!(clipped.ends_with('…'));

        let cjk = "日".repeat(200);
        assert_eq!(clip(&cjk).chars().count(), MAX_CHOICE_LEN);

        assert_eq!(clip("  short  "), "short");
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn suggests_real_tracks() {
        let http = reqwest::Client::new();
        let started = std::time::Instant::now();
        let suggestions = youtube(&http, "daft punk").await;
        let took = started.elapsed();

        println!(
            "{} suggestions in {took:?}: {suggestions:?}",
            suggestions.len()
        );

        assert!(!suggestions.is_empty(), "expected suggestions");
        assert!(suggestions.len() <= MAX_CHOICES);
        assert!(
            suggestions
                .iter()
                .all(|s| s.chars().count() <= MAX_CHOICE_LEN)
        );
        assert!(
            took < Duration::from_millis(3_000),
            "too slow for Discord's autocomplete budget: {took:?}"
        );
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn non_ascii_queries_come_back_intact() {
        let http = reqwest::Client::new();
        let suggestions = youtube(&http, "日本").await;

        println!("{suggestions:?}");
        assert!(
            suggestions.iter().any(|s| s.contains('日')),
            "expected the query's own script back, got {suggestions:?}"
        );
    }
}
