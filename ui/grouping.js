'use strict';
(function (root) {
  function groupApps(apps, order = []) {
    const sections = new Map();
    for (const app of apps) {
      const name = (app.group || '').trim() || 'Прочее';
      if (!sections.has(name)) sections.set(name, []);
      sections.get(name).push(app);
    }
    const names = [...new Set([...order, ...sections.keys()])];
    return names.filter(name => sections.has(name)).map(name => ({name, apps: sections.get(name)}));
  }
  if (typeof module !== 'undefined' && module.exports) module.exports = {groupApps};
  else root.LaunchpadGroups = {groupApps};
})(globalThis);
