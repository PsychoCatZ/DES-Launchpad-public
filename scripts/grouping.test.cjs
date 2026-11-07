'use strict';
const {test} = require('node:test');
const assert = require('node:assert/strict');
const {groupApps} = require('../ui/grouping.js');
test('explicit section order, tile order and unknown sections are preserved', () => {
  const apps = [{id:'a',group:'Почта'}, {id:'b',group:'НИИ ИИ'}, {id:'c',group:'Почта'}, {id:'d',group:'Своя группа'}];
  const result = groupApps(apps, ['НИИ ИИ', 'Пустой раздел', 'Почта']);
  assert.deepEqual(result.map(s => s.name), ['НИИ ИИ', 'Почта', 'Своя группа']);
  assert.deepEqual(result[1].apps.map(a => a.id), ['a','c']);
  assert.equal(result.flatMap(s => s.apps).length, apps.length);
});
test('old configuration remains visible under Other', () => {
  assert.equal(groupApps([{id:'a'}, {id:'b',group:'  '}])[0].apps.length, 2);
  assert.equal(groupApps([{id:'a'}])[0].name, 'Прочее');
});
test('section names cannot overwrite object properties or create empty sections', () => {
  assert.deepEqual(groupApps([{id:'a',group:'__proto__'}], ['Empty','__proto__','__proto__']).map(s=>s.name), ['__proto__']);
  assert.deepEqual(groupApps([], ['Почта']), []);
});
