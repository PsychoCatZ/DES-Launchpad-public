'use strict';
const $ = id => document.getElementById(id);
const invoke = (command, args) => window.__TAURI__.core.invoke(command, args);
let view;
const systemTheme = window.matchMedia('(prefers-color-scheme: dark)');
function applyTheme(theme) {
  document.documentElement.dataset.theme = theme === 'system' ? (systemTheme.matches ? 'dark' : 'light') : theme;
}
systemTheme.addEventListener('change', () => { if (view) applyTheme(view.settings.theme); });
function notice(message) { $('notice-text').textContent = String(message); $('notice').hidden = false; }
$('dismiss').onclick = () => { $('notice').hidden = true; };
function showSettings(open) {
  $('settings').hidden = !open;
  $('launcher').hidden = open;
  $('settings-toggle').setAttribute('aria-expanded', String(open));
  if (open && view) fillForm();
}
function fillForm() {
  const s = view.settings;
  $('settings-theme').value = s.theme;
  $('browser').value = s.browser;
  $('autostart').checked = view.autostart;
  $('compact').checked = s.compact;
  $('hide-after-launch').checked = s.hideAfterLaunch;
  $('hotkey').value = s.hotkey;
  $('config-path').textContent = view.configPath;
  $('save-status').textContent = '';
}
function fallback() { const el = document.createElement('span'); el.className = 'fallback'; el.textContent = 'DES'; el.setAttribute('aria-hidden', 'true'); return el; }
function render(next) {
  view = next;
  applyTheme(view.settings.theme);
  document.body.classList.toggle('compact', view.settings.compact);
  $('theme').value = view.settings.theme;
  const noun = {one: 'приложение', few: 'приложения', many: 'приложений', other: 'приложения'}[new Intl.PluralRules('ru').select(view.apps.length)];
  $('count').textContent = `${view.apps.length} ${noun}`;
  $('empty').hidden = view.apps.length > 0;
  $('grid').replaceChildren();
  const sectionGrids = new Map();
  for (const group of LaunchpadGroups.groupApps(view.apps, view.groups)) {
    const section = document.createElement('section'); section.className = 'app-group';
    const heading = document.createElement('h3'); heading.className = 'group-heading';
    const title = document.createElement('span'); title.textContent = group.name;
    const count = document.createElement('span'); count.className = 'group-count'; count.textContent = String(group.apps.length);
    heading.append(title, count); section.append(heading);
    const grid = document.createElement('div'); grid.className = 'grid'; section.append(grid);
    $('grid').append(section); sectionGrids.set(group.name, grid);
  }
  for (const app of view.apps) {
    const tile = document.createElement('button');
    tile.className = 'tile'; tile.type = 'button'; tile.dataset.id = app.id;
    tile.title = app.description || app.name;
    if (view.icons[app.id]) {
      const img = document.createElement('img'); img.className = 'tile-icon'; img.src = view.icons[app.id]; img.alt = '';
      img.onerror = () => img.replaceWith(fallback()); tile.append(img);
    } else tile.append(fallback());
    const copy = document.createElement('span'); copy.className = 'tile-copy';
    const name = document.createElement('span'); name.className = 'tile-name'; name.textContent = app.name;
    const desc = document.createElement('span'); desc.className = 'tile-desc';
    desc.textContent = app.description || (app.type === 'web' ? 'Веб-приложение' : 'Приложение Windows');
    copy.append(name, desc); tile.append(copy);
    const arrow = document.createElement('span'); arrow.className = 'arrow'; arrow.textContent = '↗'; arrow.setAttribute('aria-hidden', 'true'); tile.append(arrow);
    tile.onclick = async () => {
      tile.disabled = true; tile.setAttribute('aria-busy', 'true');
      try { await invoke('launch', {id: app.id}); }
      catch (error) { notice(error); }
      finally { tile.disabled = false; tile.removeAttribute('aria-busy'); }
    };
    sectionGrids.get((app.group || '').trim() || 'Прочее').append(tile);
  }
  fillForm();
  if (view.warning) notice(view.warning);
}
$('settings-toggle').onclick = () => showSettings($('settings').hidden);
$('back').onclick = () => showSettings(false);
$('theme').onchange = async () => {
  if (!view) return;
  try { render(await invoke('save_settings', { settings: {...view.settings, theme: $('theme').value}, autostart: view.autostart })); }
  catch (error) { $('theme').value = view.settings.theme; notice(error); }
};
$('settings-form').onsubmit = async event => {
  event.preventDefault(); $('save').disabled = true;
  try {
    const settings = {theme: $('settings-theme').value, browser: $('browser').value, compact: $('compact').checked, hideAfterLaunch: $('hide-after-launch').checked, hotkey: $('hotkey').value.trim()};
    render(await invoke('save_settings', {settings, autostart: $('autostart').checked}));
    $('save-status').textContent = 'Сохранено';
  } catch (error) { notice(error); }
  finally { $('save').disabled = false; }
};
$('reload').onclick = async () => {try { render(await invoke('reload_config')); notice('Конфигурация перезагружена.'); } catch (error) {notice(error);} };
for (const [id, kind] of [['open-config','config'], ['open-folder','folder'], ['open-log','log']]) {
  $(id).onclick = async () => {try {await invoke('open_data_file', {kind});} catch (error) {notice(error);} };
}
document.addEventListener('keydown', e => {if(e.key === 'Escape' && !$('settings').hidden) showSettings(false);});
(async () => {
  try {
    await window.__TAURI__.event.listen('open-settings', () => showSettings(true));
    render(await invoke('get_view'));
  } catch (error) { notice(`Не удалось загрузить панель. ${String(error)}`); }
})();
