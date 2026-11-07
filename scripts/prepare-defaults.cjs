'use strict';
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '..');
const defaults = path.join(root, 'local');
fs.mkdirSync(path.join(defaults, 'icons'), {recursive: true});
fs.mkdirSync(path.join(defaults, 'shortcuts'), {recursive: true});
try {
  fs.copyFileSync(path.join(root, 'examples', 'apps.json'), path.join(defaults, 'apps.json'), fs.constants.COPYFILE_EXCL);
  console.log('Created empty local/apps.json. Add your own apps following README.md.');
} catch (error) {
  if (error.code !== 'EEXIST') throw error;
}
