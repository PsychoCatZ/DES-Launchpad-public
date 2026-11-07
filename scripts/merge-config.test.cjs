'use strict';
const {test} = require('node:test');
const assert = require('node:assert/strict');
const {mergeConfig} = require('./merge-config.cjs');
test('upgrade keeps user targets, names and order, and adds missing applications once', () => {
  const old = {apps: [{id:'a',name:'My name',target:'user.lnk',order:9}], custom:'keep'};
  const incoming = {groups:['НИИ ИИ','Почта'], apps:[{id:'a',name:'Default',target:'default.lnk',group:'НИИ ИИ'},{id:'b',name:'Mail',group:'Почта'}]};
  const result = mergeConfig(old, incoming);
  assert.equal(result.apps[0].name, 'My name'); assert.equal(result.apps[0].target, 'user.lnk');
  assert.equal(result.apps[0].order, 9); assert.equal(result.apps[0].group, 'НИИ ИИ');
  assert.equal(result.apps[1].order, 10); assert.equal(result.custom, 'keep');
  assert.deepEqual(mergeConfig(result, incoming), result);
  assert.equal(old.apps[0].group, undefined);
});
test('custom user groups and their order survive upgrades', () => {
  const result = mergeConfig({groups:['Свои'],apps:[{id:'a',group:'Свои'}]}, {groups:['Почта'],apps:[{id:'a',group:'Почта'}]});
  assert.equal(result.apps[0].group, 'Свои'); assert.deepEqual(result.groups,['Свои','Почта']);
});
test('duplicate IDs stop the upgrade before any changes', () => {
  assert.throws(() => mergeConfig({apps:[]}, {apps:[{id:'a'},{id:'a'}]}), /Duplicate/);
});
