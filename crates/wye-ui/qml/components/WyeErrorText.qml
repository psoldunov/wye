// The words for a failed call, so they can be translated: a backend sets
// `errorKind` (a kind from crates/wye-ui/src/error_text.rs) and `error` (the
// service's own detail, or the whole message when there is no kind).
//
//   WyeErrorText { id: errors }
//   text: errors.describe(backend.errorKind, backend.error)
import QtQml

QtObject {
    // `detail` after a sentence, when there is one.
    function withDetail(sentence: string, detail: string): string {
        return detail === "" ? sentence : sentence + " " + detail;
    }

    function describe(kind: string, detail: string): string {
        switch (kind) {
        case "read-only":
            return withDetail(qsTr("Wye cannot change this setting: the file is read-only."), detail);
        case "not-lossless":
            return withDetail(qsTr("Wye did not save the change because it would drop values the configuration file contains."), detail);
        case "conflict":
            return qsTr("The configuration changed while you were editing it. Try again.");
        case "changed-elsewhere":
            return qsTr("This list changed elsewhere, so Wye did not save your change. It now shows the list as it is; try again.");
        case "refused":
            return qsTr("The service refused the change: %1").arg(detail);
        case "unavailable":
            return qsTr("Not available in this session: %1").arg(detail);
        case "unreachable":
            return qsTr("Cannot reach the Wye service: %1").arg(detail);
        case "entry-gone":
            return qsTr("That entry is gone.");
        default:
            return detail;
        }
    }
}
