// The Apps page (06-apps.md): routes links to well-known web services. A callout, then one card, titled with the page's
// heading, with a target row per service, sorted by name. APP-01 to APP-10.
pragma ComponentBehavior: Bound
import QtQuick
import dev.soldunov.wye.ui

WyePage {
    id: page

    // The catalogue only: IDs, names and installed apps. It changes when the inventory does, not on every mapping, so the
    // rows (and a menu or chooser one of them has open) are not rebuilt when a target is chosen.
    readonly property string catalogueKey: JSON.stringify(page.services.services.map(service => [service.id, service.name, service.installedApp?.id ?? ""]))
    // APP-03: alphabetical by service name.
    property var sortedServices: []

    // APP-04, APP-06: the target each service is mapped to. The backend keeps it in step with the configuration
    // (`apps.<id>`, absent for Default), so a choice shows at once.
    function targetOf(id) {
        const service = page.services.services.find(entry => entry.id === id);
        return service?.target ?? {
            "default": true
        };
    }

    function sortServices() {
        sortedServices = page.services.services.map(service => ({
                    "id": service.id,
                    "name": service.name
                })).sort((a, b) => a.name.localeCompare(b.name));
    }

    title: qsTr("Apps")

    onCatalogueKeyChanged: sortServices()
    Component.onCompleted: sortServices()

    // APP-01
    WyeCallout {
        calloutId: "apps-read-first"
        text: qsTr("This lets you open links <b>to</b> certain websites directly in their desktop app or in a specific browser. To open links <b>clicked in</b> a certain app in a specific browser, create a custom rule with “Source Apps” matching.")
        title: qsTr("Please Read")
    }

    // APP-02: the heading is the card's title. APP-03 to APP-06, APP-10: every row starts at Default (<primary>); a mapping
    // to a missing app shows a warning.
    WyeGroupCard {
        title: qsTr("Open links to web apps in their desktop app or a specific browser")

        Repeater {
            model: page.sortedServices

            WyeTargetRow {
                required property var modelData

                current: page.targetOf(modelData.id)
                service: modelData.id
                surface: "apps"
                title: modelData.name
            }
        }
    }
}
