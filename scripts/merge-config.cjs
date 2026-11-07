'use strict';
function mergeConfig(current, incoming) {
  for (const config of [current, incoming]) {
    if (!config || !Array.isArray(config.apps)) throw new Error('Configuration must contain apps');
    const ids = new Set();
    for (const app of config.apps) {
      if (!app.id || ids.has(app.id)) throw new Error('Duplicate or empty application ID');
      ids.add(app.id);
    }
  }
  const defaults = new Map(incoming.apps.map(app => [app.id, app]));
  const existing = new Set(current.apps.map(app => app.id));
  const apps = current.apps.map(app => ({...app, group: app.group?.trim() ? app.group : (defaults.get(app.id)?.group || '')}));
  let order = Math.max(-1, ...apps.map(app => Number(app.order) || 0));
  for (const app of incoming.apps) if (!existing.has(app.id)) apps.push({...app, order: ++order});
  return {...current, groups: [...new Set([...(current.groups || []), ...(incoming.groups || [])])], apps};
}
module.exports = {mergeConfig};
