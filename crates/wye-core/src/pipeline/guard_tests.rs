//! DEF-08 and DEF-09: rules and web app mappings that would send a link
//! back to the app it came from, or a sign-in page to an app, are passed
//! over.

use super::*;

use crate::keys::Modifier;
use crate::matcher::{MatcherKind, UrlMatcher};
use crate::rule::Rule;
use crate::target::{CustomApp, DesktopId};

/// Every target is available; `browsers` lists the apps that are web
/// browsers, besides the profiles and private windows the default covers.
#[derive(Default)]
struct Apps {
    browsers: Vec<Target>,
}

impl Availability for Apps {
    fn is_available(&self, _target: &Target) -> bool {
        true
    }

    fn is_browser(&self, target: &Target) -> bool {
        self.browsers.contains(target)
            || matches!(target, Target::Private(_) | Target::Profile { .. })
    }
}

fn desktop_id(id: &str) -> DesktopId {
    DesktopId::new(id).unwrap()
}

fn app(id: &str) -> Target {
    Target::App(desktop_id(id))
}

fn firefox() -> Target {
    app("firefox.desktop")
}

fn chromium() -> Target {
    app("chromium.desktop")
}

fn figma() -> Target {
    Target::Custom(CustomApp::Desktop(desktop_id("figma.desktop")))
}

fn linear() -> Target {
    Target::Custom(CustomApp::Desktop(desktop_id("linear.desktop")))
}

/// Firefox is the primary browser, Chromium the alternative one, and Figma
/// opens in its own app.
fn config() -> Config {
    let mut config = Config::default();
    config.browsers.primary = firefox();
    config.browsers.alternative = chromium();
    config.apps.insert("figma".to_owned(), figma());
    config
}

fn rule(name: &str, target: Target) -> Rule {
    Rule {
        id: None,
        name: name.to_owned(),
        enabled: true,
        target,
        url_matchers: vec![UrlMatcher {
            kind: MatcherKind::Domain,
            pattern: "figma.com".to_owned(),
        }],
        source_apps: Vec::new(),
        held_keys: Modifiers::NONE,
        open_in_background: false,
        force_new_window: false,
        run: RunPosition::Before,
        transform: false,
    }
}

fn source(desktop_id: Option<&str>, executable: Option<&str>) -> SourceApp {
    SourceApp {
        desktop_id: desktop_id.map(|id| DesktopId::new(id).unwrap()),
        executable: executable.map(str::to_owned),
    }
}

fn request(url: &str, entry: EntryPoint, source: SourceApp) -> LinkRequest {
    let mut request = LinkRequest::new(url, entry);
    request.source = source;
    request
}

fn resolve(config: Config, request: &LinkRequest, apps: &Apps) -> Resolution {
    Pipeline::with_shipped_data(config)
        .resolve(request, apps)
        .unwrap()
}

fn skipped_mapping(resolution: &Resolution, wanted: SkipReason) -> bool {
    resolution
        .steps
        .iter()
        .any(|step| matches!(step, Step::MappingSkipped { reason, .. } if *reason == wanted))
}

fn mapping_applies(resolution: &Resolution, target: &Target) -> bool {
    resolution.target == *target
        && matches!(&resolution.decision, Decision::Mapping { service } if service == "figma")
}

const FILE: &str = "https://www.figma.com/design/abc/File";
const APP_AUTH: &str = "https://www.figma.com/app_auth/x/grant?desktop_protocol=figma";

fn handler(url: &str, from: SourceApp) -> LinkRequest {
    request(url, EntryPoint::Handler, from)
}

#[test]
fn a_link_from_the_mapped_app_goes_to_the_primary_browser() {
    let from = source(Some("figma.desktop"), None);
    let resolution = resolve(config(), &handler(FILE, from), &Apps::default());
    assert_eq!(resolution.target, firefox());
    assert_eq!(resolution.decision, Decision::Fallback);
    assert!(skipped_mapping(&resolution, SkipReason::BackToSource));
}

#[test]
fn an_unknown_source_keeps_the_mapping() {
    let resolution = resolve(
        config(),
        &handler(FILE, SourceApp::default()),
        &Apps::default(),
    );
    assert!(mapping_applies(&resolution, &figma()));
}

#[test]
fn only_the_handler_entry_counts_as_coming_from_an_app() {
    for entry in [
        EntryPoint::Extension,
        EntryPoint::Clipboard,
        EntryPoint::Cli,
    ] {
        let mut config = config();
        // The extension would otherwise force the picker over the mapping.
        config.advanced.force_picker_from_extension = false;
        let from = source(Some("figma.desktop"), None);
        let resolution = resolve(config, &request(FILE, entry, from), &Apps::default());
        assert!(mapping_applies(&resolution, &figma()), "{entry:?}");
    }
}

#[test]
fn the_source_is_recognised_by_case_executable_or_program_name() {
    let apps = Apps::default();
    let cases = [
        (config(), source(Some("Figma.desktop"), None)),
        (config(), source(None, Some("Figma"))),
        (
            {
                let mut config = config();
                let program = Target::Custom(CustomApp::Executable("/opt/figma/figma".into()));
                config.apps.insert("figma".to_owned(), program);
                config
            },
            source(None, Some("figma")),
        ),
    ];
    for (config, from) in cases {
        let resolution = resolve(config, &handler(FILE, from.clone()), &apps);
        assert_eq!(resolution.target, firefox(), "{from}");
        assert!(skipped_mapping(&resolution, SkipReason::BackToSource));
    }
}

#[test]
fn a_skipped_rule_lets_the_next_matching_rule_win() {
    let mut config = config();
    config.rules = vec![rule("Figma app", figma()), rule("Chromium", chromium())];
    let from = source(Some("figma.desktop"), None);
    let resolution = resolve(config, &handler(FILE, from), &Apps::default());
    assert_eq!(resolution.target, chromium());
    assert!(matches!(
        resolution.decision,
        Decision::Rule { index: 1, .. }
    ));
    assert!(resolution.steps.iter().any(|step| matches!(
        step,
        Step::RuleSkipped {
            index: 0,
            reason: SkipReason::BackToSource,
            ..
        }
    )));
}

#[test]
fn a_browser_is_never_skipped_even_when_it_is_the_source() {
    let apps = Apps {
        browsers: vec![app("zen.desktop")],
    };
    let mut config = config();
    config.apps.insert("figma".to_owned(), app("zen.desktop"));
    let from = source(Some("zen.desktop"), None);
    let resolution = resolve(config, &handler(FILE, from), &apps);
    assert!(mapping_applies(&resolution, &app("zen.desktop")));

    let work = Target::Profile {
        app: desktop_id("firefox.desktop"),
        id: "work".into(),
    };
    let mut config = config_with_figma(work.clone());
    config.browsers.primary = chromium();
    let from = source(Some("firefox.desktop"), None);
    let resolution = resolve(config, &handler(FILE, from), &apps);
    assert!(mapping_applies(&resolution, &work));
}

fn config_with_figma(target: Target) -> Config {
    let mut config = config();
    config.apps.insert("figma".to_owned(), target);
    config
}

#[test]
fn a_sign_in_page_stays_out_of_the_app() {
    let resolution = resolve(
        config(),
        &handler(APP_AUTH, SourceApp::default()),
        &Apps::default(),
    );
    assert_eq!(resolution.target, firefox());
    assert_eq!(resolution.decision, Decision::Fallback);
    assert!(skipped_mapping(&resolution, SkipReason::SignInPage));
}

#[test]
fn a_sign_in_page_can_still_go_to_a_profile_or_the_picker() {
    let work = Target::Profile {
        app: desktop_id("firefox.desktop"),
        id: "work".into(),
    };
    for target in [work, Target::Picker] {
        let resolution = resolve(
            config_with_figma(target.clone()),
            &handler(APP_AUTH, SourceApp::default()),
            &Apps::default(),
        );
        assert!(mapping_applies(&resolution, &target), "{target}");
    }
}

#[test]
fn the_sign_in_guard_applies_to_every_entry_point() {
    for entry in [
        EntryPoint::Handler,
        EntryPoint::Clipboard,
        EntryPoint::Extension,
        EntryPoint::Cli,
    ] {
        let resolution = resolve(
            config(),
            &request(APP_AUTH, entry, SourceApp::default()),
            &Apps::default(),
        );
        assert!(
            skipped_mapping(&resolution, SkipReason::SignInPage),
            "{entry:?}"
        );
    }
}

#[test]
fn a_rule_for_an_app_is_skipped_on_a_sign_in_page() {
    let mut config = config();
    config.rules = vec![rule("Figma app", figma())];
    let resolution = resolve(
        config,
        &handler(APP_AUTH, SourceApp::default()),
        &Apps::default(),
    );
    assert_eq!(resolution.target, firefox());
    assert!(resolution.steps.iter().any(|step| matches!(
        step,
        Step::RuleSkipped {
            index: 0,
            reason: SkipReason::SignInPage,
            ..
        }
    )));
}

#[test]
fn an_oauth_link_with_a_loopback_callback_goes_to_a_browser() {
    let mut config = config();
    config.apps.insert("linear".to_owned(), linear());
    let link = "https://linear.app/oauth/authorize?client_id=x\
                &redirect_uri=http%3A%2F%2F127.0.0.1%3A48752%2Fcallback";
    let from = source(Some("org.example.Ensemblr.desktop"), None);
    let resolution = resolve(config, &handler(link, from), &Apps::default());
    assert_eq!(resolution.target, firefox());
    assert_eq!(resolution.decision, Decision::Fallback);
    assert!(skipped_mapping(&resolution, SkipReason::SignInPage));
}

#[test]
fn the_alternative_key_and_the_fallback_are_never_guarded() {
    let from = source(Some("chromium.desktop"), None);
    let mut held = handler(FILE, from);
    held.held = Modifiers::from_slice(&[Modifier::Shift]);
    let resolution = resolve(config(), &held, &Apps::default());
    assert_eq!(resolution.target, chromium());
    assert_eq!(resolution.decision, Decision::AlternativeKey);

    // The primary browser takes the link even when it is the source.
    let from = source(Some("firefox.desktop"), None);
    let resolution = resolve(
        config(),
        &handler("https://example.com/", from),
        &Apps::default(),
    );
    assert_eq!(resolution.target, firefox());
    assert_eq!(resolution.decision, Decision::Fallback);
}

// DEF-09: a mapping knows its service's own sign-in routes; a rule knows
// only the generic words.
#[test]
fn a_mapping_skips_its_services_own_sign_in_route_but_a_rule_does_not() {
    let link = "https://app.clickup.com/api?client_id=x";
    let clickup = Target::Custom(CustomApp::Desktop(desktop_id("clickup.desktop")));
    let mut mapped = Config::default();
    mapped.browsers.primary = firefox();
    mapped.apps.insert("clickup".to_owned(), clickup.clone());
    let resolution = resolve(
        mapped,
        &handler(link, SourceApp::default()),
        &Apps::default(),
    );
    assert_eq!(resolution.target, firefox());
    assert!(skipped_mapping(&resolution, SkipReason::SignInPage));

    let mut ruled = Config::default();
    ruled.browsers.primary = firefox();
    let mut to_app = rule("ClickUp app", clickup.clone());
    to_app.url_matchers[0].pattern = "clickup.com".to_owned();
    ruled.rules = vec![to_app];
    let resolution = resolve(
        ruled,
        &handler(link, SourceApp::default()),
        &Apps::default(),
    );
    assert_eq!(resolution.target, clickup);
    assert!(matches!(
        resolution.decision,
        Decision::Rule { index: 0, .. }
    ));
}

fn config_with_notion(target: Target) -> Config {
    let mut config = Config::default();
    config.browsers.primary = firefox();
    config.apps.insert("notion".to_owned(), target);
    config
}

fn notion_mapping_applies(resolution: &Resolution, target: &Target) -> bool {
    resolution.target == *target
        && matches!(&resolution.decision, Decision::Mapping { service } if service == "notion")
}

const NOTION_PAGE: &str = "https://www.notion.so/acme/Page-0123456789abcdef0123456789abcdef";
const NOTION_MAIL: &str = "https://mail.notion.so/inbox";
const NOTION_CONSENT: &str = "https://www.notion.so/install-integration?client_id=x";

// APP-13, DEF-09: a mapping to the Notion app takes only the pages the app
// opens; Notion Mail and OAuth consent go to a browser.
#[test]
fn a_mapping_to_the_notion_app_takes_only_its_pages() {
    let notion = app("notion.desktop");
    let resolution = resolve(
        config_with_notion(notion.clone()),
        &handler(NOTION_PAGE, SourceApp::default()),
        &Apps::default(),
    );
    assert!(notion_mapping_applies(&resolution, &notion));

    for (link, reason) in [
        (NOTION_MAIL, SkipReason::NotAppHost),
        (NOTION_CONSENT, SkipReason::SignInPage),
    ] {
        let resolution = resolve(
            config_with_notion(notion.clone()),
            &handler(link, SourceApp::default()),
            &Apps::default(),
        );
        assert_eq!(resolution.target, firefox(), "{link}");
        assert_eq!(resolution.decision, Decision::Fallback, "{link}");
        assert!(skipped_mapping(&resolution, reason), "{link}");
    }
}

// APP-13: a browser profile holds the Notion account, so it keeps every
// Notion host and sign-in page.
#[test]
fn a_mapping_to_a_profile_keeps_every_notion_link() {
    let work = Target::Profile {
        app: desktop_id("firefox.desktop"),
        id: "work".into(),
    };
    for link in [NOTION_PAGE, NOTION_MAIL, NOTION_CONSENT] {
        let resolution = resolve(
            config_with_notion(work.clone()),
            &handler(link, SourceApp::default()),
            &Apps::default(),
        );
        assert!(notion_mapping_applies(&resolution, &work), "{link}");
    }
}
