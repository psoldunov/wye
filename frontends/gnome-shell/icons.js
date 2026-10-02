// Icons the service names: an icon theme name or an absolute path
// (docs/dbus-api.md), with a fallback when the theme lacks it.
import Gio from 'gi://Gio';

/**
 * A GIcon for a theme name or an absolute path.
 *
 * @param {string|null} name
 * @param {string[]} fallbacks theme names tried after `name`
 * @returns {Gio.Icon}
 */
export function gicon(name, fallbacks = ['web-browser']) {
    if (name?.startsWith('/'))
        return new Gio.FileIcon({file: Gio.File.new_for_path(name)});
    return Gio.ThemedIcon.new_from_names([...name ? [name] : [], ...fallbacks]);
}

/**
 * A GIcon for a file shipped with the extension.
 *
 * @param {string} dir the extension's directory
 * @param {string} file
 */
export function bundled(dir, file) {
    return new Gio.FileIcon({file: Gio.File.new_for_path(`${dir}/${file}`)});
}
