pub const DEFAULT_WELCOME: &str = "{mention} just landed in **{server}** — you're member #{count}.";
pub const DEFAULT_FAREWELL: &str = "**{user}** left **{server}**. {count} left.";
pub const MAX_LEN: usize = 1_000;

pub struct Fields<'a> {
    pub user: &'a str,
    pub mention: String,
    pub server: &'a str,
    pub count: u64,
}

pub fn render(source: &str, fields: &Fields<'_>) -> String {
    let count = fields.count.to_string();

    let mut out = String::with_capacity(source.len());
    let mut rest = source;

    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        rest = &rest[open..];

        let Some(close) = rest.find('}') else {
            break;
        };

        let replacement = match &rest[1..close] {
            "user" => Some(fields.user),
            "mention" => Some(fields.mention.as_str()),
            "server" => Some(fields.server),
            "count" => Some(count.as_str()),
            _ => None,
        };

        match replacement {
            Some(value) => out.push_str(value),
            None => out.push_str(&rest[..=close]),
        }

        rest = &rest[close + 1..];
    }

    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> Fields<'static> {
        Fields {
            user: "yuri",
            mention: "<@123>".to_string(),
            server: "The Server",
            count: 42,
        }
    }

    #[test]
    fn fills_every_placeholder() {
        assert_eq!(
            render(
                "{mention} joined {server} as #{count}, welcome {user}",
                &fields()
            ),
            "<@123> joined The Server as #42, welcome yuri"
        );
    }

    #[test]
    fn the_defaults_render() {
        assert_eq!(
            render(DEFAULT_WELCOME, &fields()),
            "<@123> just landed in **The Server** — you're member #42."
        );
        assert_eq!(
            render(DEFAULT_FAREWELL, &fields()),
            "**yuri** left **The Server**. 42 left."
        );
    }

    #[test]
    fn unknown_placeholders_are_left_alone() {
        assert_eq!(
            render("hello {nobody} and {user}", &fields()),
            "hello {nobody} and yuri"
        );
    }

    #[test]
    fn unclosed_braces_do_not_eat_the_rest() {
        assert_eq!(render("welcome {user", &fields()), "welcome {user");
        assert_eq!(render("{", &fields()), "{");
        assert_eq!(render("a { b } c", &fields()), "a { b } c");
    }

    #[test]
    fn text_without_placeholders_passes_through() {
        assert_eq!(render("just a plain line", &fields()), "just a plain line");
        assert_eq!(render("", &fields()), "");
    }

    #[test]
    fn repeated_placeholders_all_resolve() {
        assert_eq!(render("{user} {user} {user}", &fields()), "yuri yuri yuri");
    }

    #[test]
    fn a_name_that_looks_like_a_placeholder_is_not_re_expanded() {
        let sneaky = Fields {
            user: "{server}",
            ..fields()
        };

        assert_eq!(
            render("{user}", &sneaky),
            "{server}",
            "substituted text must not be scanned again"
        );
    }

    #[test]
    fn non_ascii_survives() {
        let fields = Fields {
            user: "日本語",
            server: "サーバー",
            ..fields()
        };

        assert_eq!(render("{user} → {server}", &fields), "日本語 → サーバー");
    }
}
