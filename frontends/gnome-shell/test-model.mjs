import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {parsePicker, parseTray, keyAction, choiceOptions} from './model.mjs';

let pickerCases, trayCases;
try {
    pickerCases = JSON.parse(readFileSync(new URL('../../crates/wye-ui/fixtures/picker.json', import.meta.url))).cases;
    trayCases = JSON.parse(readFileSync(new URL('../../crates/wye-ui/fixtures/tray-menu.json', import.meta.url))).cases;
} catch (error) {
    throw new Error('Cannot load Wye UI fixtures', {cause: error});
}
for (const {argument} of pickerCases.filter(c => c.action === 'show')) {
    const picker = parsePicker(JSON.stringify(argument));
    assert.equal(picker.tiles.length, argument.tiles.length);
    assert.equal(picker.overflow.length, argument.overflow.length);
}
for (const {argument} of trayCases)
    assert.equal(parseTray(JSON.stringify(argument)).items.length, argument.items.length);
assert.equal(keyAction({actions: {next: ['Right'], previous: ['Shift+Tab']}}, 'Shift+Tab'), 'previous');
assert.equal(keyAction({actions: {next: ['Right']}}, 'Right'), 'next');
assert.deepEqual(choiceOptions({private: true, background: true}, {private: false, background: true, newWindow: false}), {background: true});
assert.throws(() => parsePicker('{"tiles":[{"name":"bad"}]}'));
assert.throws(() => parseTray('{"items":[{"kind":"action"}]}'));
