// The Extras page (09-extras.md): link hygiene applied to opened links and, optionally, to copied ones. EXT-01 to EXT-05.
// The rows that rewrite copied text need clipboard watching (EXT-12); where the session has none they are disabled and say why
// (spec 19, "Clipboard features").
pragma ComponentBehavior: Bound
import QtQuick
import dev.soldunov.wye.ui

WyePage {
    id: page

    readonly property bool watching: SettingsBackend.clipboardWatchAvailable
    readonly property string unavailable: qsTr("Not available in this session")

    title: qsTr("Extras")

    WyeGroupCard {
        // EXT-01: the help popover lists what is removed (EXT-10).
        WyeSwitchRow {
            help: "remove-tracking"
            isOn: page.value(path, true)
            path: "extras.strip-tracking-on-open"
            title: qsTr("Remove tracking parameters when opening links")
        }

        // EXT-02
        WyeSwitchRow {
            dimmed: !page.watching
            help: page.watching ? "" : "clipboard-unavailable"
            isOn: page.value(path, false)
            path: "extras.strip-tracking-on-copy"
            subtitle: page.watching ? "" : page.unavailable
            title: qsTr("Remove tracking parameters when copying links")
        }

        // EXT-03
        WyeSwitchRow {
            dimmed: !page.watching
            help: page.watching ? "" : "clipboard-unavailable"
            isOn: page.value(path, false)
            path: "extras.strip-mailto-on-copy"
            subtitle: page.watching ? "" : page.unavailable
            title: qsTr("Remove leading “mailto:” when copying email addresses")
        }
    }

    // EXT-04
    WyeGroupCard {
        WyeSwitchRow {
            isOn: page.value(path, false)
            path: "extras.force-https"
            title: qsTr("Force opened links to be HTTPS")
        }
    }

    // EXT-05, EXT-15: "Songlink" is a link to song.link.
    WyeGroupCard {
        WyeSwitchRow {
            dimmed: !page.watching
            help: page.watching ? "" : "clipboard-unavailable"
            isOn: page.value(path, false)
            path: "extras.songlink-on-copy"
            subtitle: page.watching ? qsTr("For easy sharing with anyone, regardless of the music service they use. Supports Apple Music, Spotify, TIDAL, and Deezer.") : page.unavailable
            title: qsTr("Convert copied music links to <a href=\"https://song.link\">Songlink</a>")
            titleFormat: Text.RichText
        }
    }
}
