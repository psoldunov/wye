//! The script's globals (SCR-20): the JavaScript prelude in `prelude.js`
//! and the native functions it is built on.
//!
//! Every URL operation goes through [`url::quirks`], the `url` crate's
//! implementation of the WHATWG URL API, so getters and setters behave as
//! in a browser. Nothing else is exposed: no network, no files, no timers.

use rquickjs::function::Opt;
use rquickjs::{Ctx, Function, Object};
use url::{Url, form_urlencoded, quirks};

/// The prelude's source; evaluates to a function taking the natives.
const SOURCE: &str = include_str!("prelude.js");

/// What the engine keeps from the prelude.
pub(crate) struct Helpers<'js> {
    /// `href → new URL(href)`.
    pub make: Function<'js>,
    /// `value → value.href` when `value` is one of the prelude's `URL`s,
    /// else `undefined`.
    pub href_of: Function<'js>,
}

/// Define `URL`, `URLSearchParams` and `console` in `ctx`; `log` receives
/// each `console.log` line.
///
/// # Errors
///
/// When the prelude cannot be evaluated, which only happens when the
/// runtime is out of memory.
pub(crate) fn install<'js>(
    ctx: &Ctx<'js>,
    log: impl Fn(String) + 'js,
) -> rquickjs::Result<Helpers<'js>> {
    let natives = Object::new(ctx.clone())?;
    // QuickJS hands over owned values; the helpers borrow them.
    let natives_parse = |input: String, base: Opt<String>| parse(&input, base.0.as_deref());
    let natives_parts = |href: String| parts(&href);
    let natives_update = |href: String, name: String, value: String| update(&href, &name, &value);
    let natives_form_parse = |query: String| form_parse(&query);
    let natives_form_serialize = |pairs: Vec<Vec<String>>| form_serialize(&pairs);
    natives.set("parse", Function::new(ctx.clone(), natives_parse)?)?;
    natives.set("parts", Function::new(ctx.clone(), natives_parts)?)?;
    natives.set("update", Function::new(ctx.clone(), natives_update)?)?;
    natives.set("formParse", Function::new(ctx.clone(), natives_form_parse)?)?;
    natives.set(
        "formSerialize",
        Function::new(ctx.clone(), natives_form_serialize)?,
    )?;
    natives.set(
        "log",
        Function::new(ctx.clone(), move |line: String| log(line))?,
    )?;
    let setup: Function = ctx.eval(SOURCE)?;
    let helpers: Object = setup.call((natives,))?;
    Ok(Helpers {
        make: helpers.get("make")?,
        href_of: helpers.get("hrefOf")?,
    })
}

/// `new URL(input, base)`: the serialised URL, or `None` when it does not
/// parse.
fn parse(input: &str, base: Option<&str>) -> Option<String> {
    let parsed = match base {
        Some(base) => Url::parse(base).and_then(|base| base.join(input)),
        None => Url::parse(input),
    };
    parsed.ok().map(String::from)
}

/// The URL's attributes, in the order `prelude.js` reads them.
fn parts(href: &str) -> Option<Vec<String>> {
    let url = Url::parse(href).ok()?;
    Some(vec![
        quirks::href(&url).to_owned(),
        quirks::origin(&url),
        quirks::protocol(&url).to_owned(),
        quirks::username(&url).to_owned(),
        quirks::password(&url).to_owned(),
        quirks::host(&url).to_owned(),
        quirks::hostname(&url).to_owned(),
        quirks::port(&url).to_owned(),
        quirks::pathname(&url).to_owned(),
        quirks::search(&url).to_owned(),
        quirks::hash(&url).to_owned(),
    ])
}

/// The URL after setting attribute `name` to `value`. `None` for an
/// unknown attribute or an `href` that does not parse; WHATWG setters other
/// than `href` ignore values they cannot use, and so does this.
fn update(href: &str, name: &str, value: &str) -> Option<String> {
    let mut url = Url::parse(href).ok()?;
    if name == "href" {
        quirks::set_href(&mut url, value).ok()?;
    } else {
        set_attribute(&mut url, name, value)?;
    }
    Some(url.into())
}

/// Set one attribute other than `href`; `None` when `name` is unknown.
fn set_attribute(url: &mut Url, name: &str, value: &str) -> Option<()> {
    // A setter's `Err(())` means "value ignored", which is what WHATWG
    // setters do with a value they cannot use; the URL stays as it was.
    let _ignored = match name {
        "protocol" => quirks::set_protocol(url, value),
        "username" => quirks::set_username(url, value),
        "password" => quirks::set_password(url, value),
        "host" => quirks::set_host(url, value),
        "hostname" => quirks::set_hostname(url, value),
        "port" => quirks::set_port(url, value),
        "pathname" => {
            quirks::set_pathname(url, value);
            Ok(())
        }
        "search" => {
            quirks::set_search(url, value);
            Ok(())
        }
        "hash" => {
            quirks::set_hash(url, value);
            Ok(())
        }
        _ => return None,
    };
    Some(())
}

/// `application/x-www-form-urlencoded` parsing, as pairs.
fn form_parse(query: &str) -> Vec<Vec<String>> {
    form_urlencoded::parse(query.as_bytes())
        .map(|(name, value)| vec![name.into_owned(), value.into_owned()])
        .collect()
}

/// The pairs, serialised as `application/x-www-form-urlencoded`.
fn form_serialize(pairs: &[Vec<String>]) -> String {
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    for pair in pairs {
        if let [name, value] = pair.as_slice() {
            serializer.append_pair(name, value);
        }
    }
    serializer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_resolves_against_a_base() {
        assert_eq!(
            parse("/a?b", Some("https://example.com/x/y")).as_deref(),
            Some("https://example.com/a?b")
        );
        assert_eq!(parse("not a url", None), None);
    }

    #[test]
    fn update_follows_the_whatwg_setters() {
        let href = "https://twitter.com/a?x=1#h";
        assert_eq!(
            update(href, "hostname", "x.com").as_deref(),
            Some("https://x.com/a?x=1#h")
        );
        // An invalid port is ignored, not an error.
        assert_eq!(update(href, "port", "nope").as_deref(), Some(href));
        assert_eq!(update(href, "href", "::"), None);
        assert_eq!(update(href, "colour", "red"), None);
    }

    #[test]
    fn forms_round_trip() {
        let pairs = form_parse("a=1&b=x+y&c=%26");
        assert_eq!(pairs, [["a", "1"], ["b", "x y"], ["c", "&"]]);
        assert_eq!(form_serialize(&pairs), "a=1&b=x+y&c=%26");
    }
}
