'use strict';

const icons = {
  overview: '<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>',
  processes: '<path d="M4 6h16M4 12h16M4 18h16"/><path d="M8 4v4M16 10v4M10 16v4"/>',
  cleanup: '<path d="m14 3 7 7-8 8-7-7Z"/><path d="m6 11-3 7 3 3 7-3M11 6l7 7M5 17l2 2"/>',
  journal: '<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M9 8h6M9 12h6M9 16h4"/>',
  settings: '<path d="m9 3-1 3-3 1-2 5 2 5 3 1 1 3h6l1-3 3-1 2-5-2-5-3-1-1-3Z"/><circle cx="12" cy="12" r="3"/>',
  shield: '<path d="m12 3 8 3v6c0 5-8 9-8 9s-8-4-8-9V6Z"/><path d="m8 12 3 3 5-6"/>',
  cpu: '<rect x="6" y="6" width="12" height="12" rx="2"/><rect x="9" y="9" width="6" height="6" rx="1"/><path d="M9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4"/>',
  memory: '<rect x="3" y="5" width="18" height="12" rx="2"/><path d="M7 9v4M12 9v4M17 9v4M6 17v3M10 17v3M14 17v3M18 17v3"/>',
  disk: '<rect x="3" y="4" width="18" height="16" rx="3"/><path d="M3 14h18M15 17h2"/><circle cx="7" cy="17" r=".5"/>',
  network: '<rect x="8" y="2" width="8" height="6" rx="1"/><rect x="2" y="16" width="6" height="5" rx="1"/><rect x="16" y="16" width="6" height="5" rx="1"/><path d="M12 8v4M5 16v-4h14v4"/>',
  activity: '<path d="M2 12h4l3-7 6 14 3-7h4"/>',
  pause: '<path d="M9 5v14M15 5v14"/>',
  play: '<path d="m8 4 12 8-12 8Z"/>',
  arrow: '<path d="M4 12h16m-6-6 6 6-6 6"/>',
};

function insertIcons(root = document) {
  root.querySelectorAll('[data-icon]').forEach((element) => {
    const path = icons[element.dataset.icon];
    if (path) element.innerHTML = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${path}</svg>`;
  });
}

const concepts = {
  control: { number: '01', tag: 'ГРАФИТ И МЯТА', title: 'Спокойный контроль.', description: 'Чёткая навигация, крупные показатели и внимание к следующему действию.', detail: 'Мятный акцент · боковая навигация · спокойная плотность', name: 'Control' },
  studio: { number: '02', tag: 'УГОЛЬ И ЯНТАРЬ', title: 'Больше воздуха.', description: 'Горизонтальная навигация, мягкие поверхности и ясный путь к очистке.', detail: 'Янтарный акцент · верхняя навигация · просторная композиция', name: 'Studio' },
  signal: { number: '03', tag: 'СЛАНЕЦ И ЛЁД', title: 'Каждая деталь на виду.', description: 'Плотная рабочая панель для тех, кто любит видеть всю картину.', detail: 'Голубой акцент · узкая панель · инструментальная плотность', name: 'Signal' },
};
const pages = { overview: 'Обзор', processes: 'Процессы', cleanup: 'Очистка', journal: 'Журнал', settings: 'Настройки' };

const studioPalettes = {
  amber: { number: '01', name: 'Янтарь', tag: 'УГОЛЬ И ЯНТАРЬ', description: 'Тёплый угольный фон и мягкий янтарный акцент. Спокойное пространство для обслуживания ПК.' },
  mint: { number: '02', name: 'Мята', tag: 'ЛЕС И МЯТА', description: 'Глубокие зелёные поверхности и свежая мята. Собранный, спокойный интерфейс.' },
  glacier: { number: '03', name: 'Ледник', tag: 'НОЧЬ И ЛЁД', description: 'Холодный тёмный фон и ледяной голубой акцент. Ясный, технический характер.' },
  rose: { number: '04', name: 'Роза', tag: 'ОБСИДИАН И РОЗА', description: 'Тёмные сливовые поверхности и пыльная роза. Выразительная, мягкая палитра.' },
  pearl: { number: '05', name: 'Жемчуг', tag: 'ГРАФИТ И СЕРЕБРО', description: 'Нейтральный графит и жемчужный акцент. Сдержанный интерфейс с акцентом на типографике.' },
};

function openPage(name) {
  if (!Object.hasOwn(pages, name)) return;
  document.querySelectorAll('.page').forEach((page) => { page.hidden = page.id !== `page-${name}`; });
  document.querySelectorAll('[data-page]').forEach((button) => {
    button.classList.toggle('selected', button.dataset.page === name);
    if (button.dataset.page === name) button.setAttribute('aria-current', 'page');
    else button.removeAttribute('aria-current');
  });
  document.getElementById('breadcrumb').textContent = pages[name];
}

function changeConcept(name, palette = 'amber') {
  const concept = concepts[name];
  if (!concept) return;
  document.body.dataset.concept = name;
  document.querySelectorAll('.concepts button').forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.concept === name)));
  document.getElementById('direction-tag').textContent = `${concept.number} / ${concept.tag}`;
  document.getElementById('direction-title').textContent = concept.title;
  document.getElementById('direction-description').textContent = concept.description;
  document.getElementById('direction-detail').textContent = concept.detail;
  document.getElementById('concept-number').textContent = concept.number;
  document.getElementById('theme-name').textContent = concept.name;
  const selectedPalette = Object.hasOwn(studioPalettes, palette) ? palette : 'amber';
  document.body.dataset.studioPalette = selectedPalette;
  document.querySelectorAll('[data-palette]').forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.palette === selectedPalette)));
  if (name === 'studio') {
    const color = studioPalettes[selectedPalette];
    document.getElementById('direction-tag').textContent = `STUDIO / ${color.number} / ${color.tag}`;
    document.getElementById('direction-title').textContent = `Studio. ${color.name}.`;
    document.getElementById('direction-description').textContent = color.description;
    document.getElementById('direction-detail').textContent = `${color.name} · верхняя навигация · просторная композиция`;
    document.getElementById('concept-number').textContent = `Studio / ${color.number}`;
    document.getElementById('theme-name').textContent = `Studio / ${color.name}`;
  }
  history.replaceState(null, '', name === 'studio' ? `#studio-${selectedPalette}` : `#${name}`);
}

function applyRoute() {
  const [name, palette] = location.hash.slice(1).split('-');
  changeConcept(Object.hasOwn(concepts, name) ? name : 'control', palette);
}

insertIcons();
document.querySelectorAll('.concepts button').forEach((button) => button.addEventListener('click', () => changeConcept(button.dataset.concept)));
document.querySelectorAll('[data-palette]').forEach((button) => button.addEventListener('click', () => changeConcept('studio', button.dataset.palette)));
document.querySelectorAll('[data-page]').forEach((button) => {
  button.setAttribute('aria-label', pages[button.dataset.page]);
  button.setAttribute('title', pages[button.dataset.page]);
  button.addEventListener('click', () => openPage(button.dataset.page));
});
document.querySelectorAll('[data-open]').forEach((button) => button.addEventListener('click', () => openPage(button.dataset.open)));
document.querySelector('.brand').addEventListener('click', (event) => { event.preventDefault(); openPage('overview'); });
window.addEventListener('hashchange', applyRoute);
applyRoute();

document.querySelectorAll('[data-chart]').forEach((button) => button.addEventListener('click', () => {
  document.querySelectorAll('[data-chart]').forEach((tab) => { tab.classList.toggle('active', tab === button); tab.setAttribute('aria-pressed', String(tab === button)); });
  const labels = { CPU: 'CPU / загрузка', RAM: 'RAM / использование', 'Сеть': 'Сеть / активность' };
  document.getElementById('chart-label').textContent = labels[button.dataset.chart];
}));

let paused = false;
document.getElementById('pause').addEventListener('click', (event) => {
  paused = !paused;
  const button = event.currentTarget;
  button.setAttribute('aria-pressed', String(paused));
  button.querySelector('span:last-child').textContent = paused ? 'Продолжить обновление' : 'Пауза обновления';
  button.querySelector('[data-icon]').dataset.icon = paused ? 'play' : 'pause';
  insertIcons(button);
});
document.getElementById('scan-demo').addEventListener('click', () => { document.getElementById('scan-result').hidden = false; });
document.getElementById('sort-process').addEventListener('click', (event) => {
  const button = event.currentTarget;
  button.textContent = button.textContent.includes('↓') ? 'CPU ↑' : 'CPU ↓';
});
document.getElementById('process-search').addEventListener('input', (event) => {
  document.getElementById('process-note').textContent = event.target.value.trim() ? 'Фильтр выбран. Данные для поиска появятся после подключения мониторинга.' : 'Мониторинг не подключён. Прототип показывает компоновку таблицы.';
});
