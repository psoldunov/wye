// Pure boundary model; shared by the Shell extension and the Node fixture check.
function object(value) {
    return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function decode(json, kind) {
    let value;
    try {
        value = JSON.parse(json);
    } catch (error) {
        throw new Error(`Invalid ${kind} JSON`, {cause: error});
    }
    if (!object(value))
        throw new Error(`Invalid ${kind}: expected object`);
    return value;
}

function tile(value) {
    if (!object(value) || !object(value.target) || typeof value.name !== 'string')
        throw new Error('Invalid picker tile');
    return value;
}

export function parsePicker(json) {
    const value = decode(json, 'picker');
    if (!Array.isArray(value.tiles) || !Array.isArray(value.overflow) ||
        !object(value.url) || typeof value.url.full !== 'string')
        throw new Error('Invalid picker request');
    value.tiles.forEach(tile);
    for (const group of value.overflow) {
        if (!object(group) || !Array.isArray(group.tiles))
            throw new Error('Invalid picker overflow');
        group.tiles.forEach(tile);
    }
    return value;
}

export function parseTray(json) {
    const value = decode(json, 'tray');
    if (!Array.isArray(value.items))
        throw new Error('Invalid tray items');
    function validate(items) {
        for (const item of items) {
            if (!object(item) || typeof item.kind !== 'string' || typeof item.id !== 'string')
                throw new Error('Invalid tray item');
            if (item.children !== undefined) {
                if (!Array.isArray(item.children))
                    throw new Error('Invalid tray children');
                validate(item.children);
            }
        }
    }
    validate(value.items);
    return value;
}

export function keyAction(keys, chord) {
    for (const [action, bindings] of Object.entries(keys?.actions ?? {})) {
        if (Array.isArray(bindings) && bindings.some(key => key.toLowerCase() === chord.toLowerCase()))
            return action;
    }
    return null;
}

export function choiceOptions(modifiers, capabilities = {}) {
    return Object.fromEntries(Object.entries({
        private: modifiers.private,
        background: modifiers.background,
        'new-window': modifiers.newWindow,
    }).filter(([name, active]) => active && capabilities[name === 'new-window' ? 'newWindow' : name]));
}
