'use strict';
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '..');
const defaults = path.join(root, 'local');
const config = JSON.parse(fs.readFileSync(path.join(defaults, 'apps.json'), 'utf8').replace(/^\uFEFF/, ''));
if (!Array.isArray(config.apps) || config.apps.length || (config.groups && config.groups.length)) {
  throw new Error('Public builds require empty defaults. Use a clean checkout; never package personal configuration.');
}
for (const dir of ['icons', 'shortcuts']) {
  if (fs.readdirSync(path.join(defaults, dir)).length) throw new Error(`Public defaults/${dir} must be empty.`);
}
console.log('Public defaults verified: no applications, icons or shortcuts.');
