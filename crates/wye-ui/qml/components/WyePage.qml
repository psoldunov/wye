// WyePage: the root of a settings page. A column that fills the window's width with the groups below one another, and
// the parsed configuration, status and services for the page's bindings. Pages are shown by the Settings window.
//
// API
//   title: string          the page's name; the window's title (SET-01): "General", "Browsers", …
//   config: var            the configuration (kebab-case JSON, as `config.toml`)
//   status: var            `Status` (defaultBrowser, config, capabilities, uiState)
//   services: var          `GetServices`: {services: [{id, name, icon, installedApp, target}]}
//   editable: bool         changes can be saved; false for a read-only file (home-manager). Rows disable their own control
//                          then (WyeRow.needsConfig); callouts, help buttons and links keep working.
//   value(path, fallback)  the configuration value at the dotted path, for example value("general.tray-icon", "wye")
//   default property       the page's content: WyeCallout, WyeGroupCard, WyeSection
//   overlays               sheets and dialogs the page opens, for example a Loader of a WyeSheet: they take no room in the
//                          page's layout
//   A page opens a sheet of its own by listening to SettingsBackend.sheetRequested(name); the window asks with
//   SettingsBackend.requestSheet(name).
//
//   WyePage {
//       id: page
//       title: qsTr("General")
//       WyeGroupCard { title: qsTr("Startup"); WyeSwitchRow { … isOn: page.value(path, true) } }
//   }
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.soldunov.wye.ui

ColumnLayout {
    id: page

    property string title
    property alias overlays: overlayHost.data

    readonly property var config: SettingsBackend.configJson === "" ? ({}) : JSON.parse(SettingsBackend.configJson)
    readonly property var status: SettingsBackend.statusJson === "" ? ({}) : JSON.parse(SettingsBackend.statusJson)
    readonly property var services: SettingsBackend.servicesJson === "" ? ({
            "services": []
        }) : JSON.parse(SettingsBackend.servicesJson)
    readonly property bool editable: SettingsBackend.writable

    function value(path, fallback) {
        let node = page.config;
        for (const key of path.split(".")) {
            if (node === null || typeof node !== "object" || !(key in node)) {
                return fallback;
            }
            node = node[key];
        }
        return node;
    }

    spacing: Kirigami.Units.largeSpacing

    Item {
        id: overlayHost

        visible: false
    }
}
