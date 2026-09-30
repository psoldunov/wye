use super::*;

fn tracking() -> TrackingRules {
    TrackingRules::shipped()
}

fn all() -> RewriteOptions {
    RewriteOptions {
        strip_tracking: true,
        strip_mailto: true,
        songlink: true,
    }
}

fn offer(text: &str) -> ClipboardOffer {
    ClipboardOffer {
        text: Some(text.to_owned()),
        ..ClipboardOffer::default()
    }
}

fn decide_with(text: &str, options: RewriteOptions) -> Rewrite {
    decide(&offer(text), &OwnWrites::new(), options, &tracking())
}

fn url(link: &str) -> Url {
    Url::parse(link).unwrap()
}

// EXT-12
#[test]
fn a_single_line_is_a_candidate_and_anything_else_is_not() {
    assert_eq!(
        single_line("https://example.com/\n"),
        Some("https://example.com/")
    );
    assert_eq!(
        single_line("  https://example.com/  "),
        Some("https://example.com/")
    );
    assert_eq!(single_line("one\ntwo"), None);
    assert_eq!(single_line("one\r\ntwo"), None);
    assert_eq!(single_line("   \n"), None);
    assert_eq!(single_line(""), None);
}

// TRAY-10, IN-02
#[test]
fn clipboard_link_needs_a_single_line_web_link() {
    assert_eq!(
        clipboard_link("https://example.com/a?b=1\n"),
        Some(url("https://example.com/a?b=1"))
    );
    assert!(clipboard_link("http://localhost:8080/x").is_some());
    assert!(clipboard_link("ftp://example.com/").is_none());
    assert!(clipboard_link("mailto:a@b.example").is_none());
    assert!(clipboard_link("javascript:alert(1)").is_none());
    assert!(clipboard_link("https://").is_none());
    assert!(clipboard_link("see https://example.com/ for details").is_none());
    assert!(clipboard_link("https://example.com/\nhttps://example.org/").is_none());
    assert!(clipboard_link("example.com").is_none());
    assert!(clipboard_link("").is_none());
}

// EXT-02
#[test]
fn tracking_parameters_are_removed_and_written_back() {
    let options = RewriteOptions {
        strip_tracking: true,
        ..RewriteOptions::default()
    };
    assert_eq!(
        decide_with("https://example.com/a?utm_source=x&id=7#top", options),
        Rewrite::Write("https://example.com/a?id=7#top".to_owned())
    );
    assert_eq!(
        decide_with("  https://example.com/a?fbclid=1 \n", options),
        Rewrite::Write("https://example.com/a".to_owned())
    );
}

// EXT-12: nothing to change means nothing is written, so no loop starts.
#[test]
fn a_clean_link_is_left_alone() {
    let options = RewriteOptions {
        strip_tracking: true,
        ..RewriteOptions::default()
    };
    assert_eq!(
        decide_with("https://example.com/a?id=7", options),
        Rewrite::Unchanged
    );
    assert_eq!(
        decide_with("https://example.com", options),
        Rewrite::Unchanged
    );
}

// EXT-13
#[test]
fn mailto_loses_its_prefix_and_query() {
    let options = RewriteOptions {
        strip_mailto: true,
        ..RewriteOptions::default()
    };
    for (text, expected) in [
        ("mailto:name@example.com", "name@example.com"),
        (
            "mailto:name@example.com?subject=Hello%20there&body=x",
            "name@example.com",
        ),
        ("MAILTO:Name@Example.com", "Name@Example.com"),
        (
            "mailto:a@example.com,b@example.org",
            "a@example.com,b@example.org",
        ),
        ("  mailto:name@example.com\n", "name@example.com"),
    ] {
        assert_eq!(
            decide_with(text, options),
            Rewrite::Write(expected.to_owned()),
            "{text}"
        );
    }
}

#[test]
fn text_that_is_not_a_mailto_address_is_not_touched() {
    let options = RewriteOptions {
        strip_mailto: true,
        ..RewriteOptions::default()
    };
    for text in [
        "mailto:",
        "mailto:?subject=x",
        "mailto:not-an-address",
        "mailto:@example.com",
        "mailto:a b@example.com",
        "name@example.com",
        "mailtox:a@example.com",
        "mail",
    ] {
        assert_eq!(decide_with(text, options), Rewrite::Unchanged, "{text}");
    }
    assert_eq!(
        mailto_address("mailto:a@b.example"),
        Some("a@b.example".to_owned())
    );
    assert_eq!(mailto_address("http://a@b.example"), None);
    // Multi-byte text shorter than the prefix must not panic.
    assert_eq!(mailto_address("ääääääääää"), None);
}

#[test]
fn mailto_needs_its_switch_and_is_not_cleaned_as_a_link() {
    let only_tracking = RewriteOptions {
        strip_tracking: true,
        ..RewriteOptions::default()
    };
    assert_eq!(
        decide_with("mailto:name@example.com", only_tracking),
        Rewrite::Unchanged
    );
}

// EXT-05, EXT-15
#[test]
fn a_music_link_goes_to_songlink_with_its_cleaned_form_as_the_fallback() {
    let options = RewriteOptions {
        strip_tracking: true,
        songlink: true,
        ..RewriteOptions::default()
    };
    let link = "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC?si=abc123";
    // `si` on Spotify is a tracking parameter (EXT-10).
    assert_eq!(
        decide_with(link, options),
        Rewrite::Songlink {
            query: url("https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC"),
            fallback: Some("https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC".to_owned()),
        }
    );
    let songlink_only = RewriteOptions {
        songlink: true,
        ..RewriteOptions::default()
    };
    assert_eq!(
        decide_with(link, songlink_only),
        Rewrite::Songlink {
            query: url(link),
            fallback: None,
        }
    );
}

#[test]
fn songlink_is_only_asked_for_supported_links() {
    let options = RewriteOptions {
        songlink: true,
        ..RewriteOptions::default()
    };
    assert_eq!(
        decide_with("https://example.com/track/1", options),
        Rewrite::Unchanged
    );
    assert_eq!(
        decide_with("https://open.spotify.com/user/someone", options),
        Rewrite::Unchanged
    );
}

// EXT-12
#[test]
fn the_guards_leave_the_clipboard_alone() {
    let options = all();
    let link = "https://example.com/a?utm_source=x";
    let own = OwnWrites::new().remember(link);
    assert_eq!(
        decide(&offer(link), &own, options, &tracking()),
        Rewrite::Unchanged
    );
    assert_ne!(
        decide(&offer(link), &OwnWrites::new(), options, &tracking()),
        Rewrite::Unchanged
    );

    let rich = ClipboardOffer {
        rich: true,
        ..offer(link)
    };
    assert_eq!(
        decide(&rich, &OwnWrites::new(), options, &tracking()),
        Rewrite::Unchanged
    );

    for hint in ["secret", " Secret "] {
        let secret = ClipboardOffer {
            password_manager_hint: Some(hint.to_owned()),
            ..offer(link)
        };
        assert_eq!(
            decide(&secret, &OwnWrites::new(), options, &tracking()),
            Rewrite::Unchanged,
            "{hint:?}"
        );
    }
    let other_hint = ClipboardOffer {
        password_manager_hint: Some("not-secret".to_owned()),
        ..offer(link)
    };
    assert_ne!(
        decide(&other_hint, &OwnWrites::new(), options, &tracking()),
        Rewrite::Unchanged
    );

    let empty = ClipboardOffer::default();
    assert_eq!(
        decide(&empty, &OwnWrites::new(), options, &tracking()),
        Rewrite::Unchanged
    );
    assert_eq!(decide_with("a\nb", options), Rewrite::Unchanged);
    assert_eq!(decide_with("just some words", options), Rewrite::Unchanged);
    assert_eq!(
        decide_with(link, RewriteOptions::default()),
        Rewrite::Unchanged,
        "with every switch off nothing is rewritten"
    );
}

#[test]
fn the_guard_remembers_only_the_last_write() {
    let guard = OwnWrites::new();
    assert!(!guard.is_own("x"));
    let guard = guard.remember("one");
    assert!(guard.is_own("one"));
    let guard = guard.remember("two");
    assert!(guard.is_own("two"));
    assert!(!guard.is_own("one"));
}

#[test]
fn options_come_from_the_extras_page() {
    let extras = Extras {
        strip_tracking_on_copy: true,
        songlink_on_copy: true,
        ..Extras::default()
    };
    let options = RewriteOptions::from_extras(&extras);
    assert_eq!(
        options,
        RewriteOptions {
            strip_tracking: true,
            strip_mailto: false,
            songlink: true,
        }
    );
    assert!(options.any());
    assert!(!RewriteOptions::from_extras(&Extras::default()).any());
}

// EXT-05: Apple Music, Spotify, TIDAL and Deezer.
#[test]
fn songlink_supports_tracks_and_albums_of_the_four_services() {
    for link in [
        "https://music.apple.com/us/album/some-album/1234567890",
        "https://music.apple.com/de/album/some-album/1234567890?i=1234567891",
        "https://music.apple.com/us/song/some-song/1234567891",
        "https://itunes.apple.com/us/album/some-album/id1234567890",
        "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC",
        "https://open.spotify.com/album/4uLU6hMCjMI75M1A2tKUQC?si=x",
        "https://open.spotify.com/intl-de/track/4uLU6hMCjMI75M1A2tKUQC",
        "https://tidal.com/track/12345",
        "https://tidal.com/browse/album/12345",
        "https://listen.tidal.com/album/12345/track/67890",
        "https://www.deezer.com/track/12345",
        "https://www.deezer.com/en/album/12345",
        "https://deezer.com/fr/track/12345",
    ] {
        assert!(songlink_supported(&url(link)), "{link}");
    }
}

#[test]
fn songlink_leaves_everything_else_alone() {
    for link in [
        "https://open.spotify.com/artist/4uLU6hMCjMI75M1A2tKUQC",
        "https://open.spotify.com/playlist/4uLU6hMCjMI75M1A2tKUQC",
        "https://open.spotify.com/track/",
        "https://open.spotify.com/",
        "https://music.apple.com/us/playlist/some/pl.123",
        "https://music.apple.com/us/artist/some/123",
        "https://tidal.com/browse/artist/123",
        "https://www.deezer.com/en/artist/12345",
        "https://www.deezer.com/en/playlist/12345",
        "https://song.link/s/4uLU6hMCjMI75M1A2tKUQC",
        "https://example.com/track/1",
        "https://open.spotify.com.evil.example/track/4uLU6hMCjMI75M1A2tKUQC",
        "https://notspotify.com/track/1",
        "spotify:track:4uLU6hMCjMI75M1A2tKUQC",
        "ftp://open.spotify.com/track/1",
    ] {
        assert!(!songlink_supported(&url(link)), "{link}");
    }
}

// EXT-15
#[test]
fn the_api_request_carries_the_encoded_link() {
    let request = songlink_api_url(&url("https://open.spotify.com/track/abc?si=x&y=1"));
    assert_eq!(request.host_str(), Some("api.song.link"));
    assert_eq!(request.path(), "/v1-alpha.1/links");
    let pairs: Vec<_> = request.query_pairs().collect();
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].0, "url");
    assert_eq!(pairs[0].1, "https://open.spotify.com/track/abc?si=x&y=1");
    assert!(
        request.query().unwrap().contains("%3A%2F%2F"),
        "the link is percent-encoded: {}",
        request.query().unwrap()
    );
}

// EXT-15: on any failure the clipboard stays unchanged.
#[test]
fn the_page_comes_from_the_answer_or_not_at_all() {
    let ok = r#"{"entityUniqueId":"SPOTIFY_SONG::x","pageUrl":"https://song.link/s/4uLU6hMCjMI75M1A2tKUQC","linksByPlatform":{}}"#;
    assert_eq!(
        songlink_page(ok),
        Some("https://song.link/s/4uLU6hMCjMI75M1A2tKUQC".to_owned())
    );
    let album = r#"{"pageUrl":"https://album.link/i/123"}"#;
    assert_eq!(
        songlink_page(album),
        Some("https://album.link/i/123".to_owned())
    );
    for bad in [
        "",
        "not json",
        "{}",
        r#"{"pageUrl":null}"#,
        r#"{"pageUrl":42}"#,
        r#"{"pageUrl":"not a url"}"#,
        r#"{"pageUrl":"http://song.link/s/x"}"#,
        r#"{"pageUrl":"https://evil.example/s/x"}"#,
        r#"{"pageUrl":"https://song.link.evil.example/s/x"}"#,
        r#"{"pageUrl":"javascript:alert(1)"}"#,
        r#"{"error":"not found"}"#,
    ] {
        assert_eq!(songlink_page(bad), None, "{bad}");
    }
}
