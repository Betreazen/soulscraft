'use strict';
// Interface pieces shared by every page. The look is the shared guide kit of style.css (gd-
// classes, «Кузня душ»); the behaviour is here, without a framework (patterns after NumberFlow,
// Sonner, Vaul, Radix and Magic UI):
//   SC.Flow       — a number whose digits roll to a new value (.gd-nf)
//   SC.toast      — a short note in the corner, newest in front (.gd-toasts)
//   SC.sheet      — a bottom sheet with a grip to drag it away (.gd-sheet); the nav on phones too
//   SC.copyLink   — copies a link to this page at a hash, with a toast
//   SC.readout    — the tick counter and phase chip under a 3D window
// and, on their own: the pointer light on .gd-glow cards, sparks that rest while a 3D window is on
// screen, and Radix-like keyboard and ARIA for the button rows (.sc-clips): arrows move, one tab
// stop per row, aria-pressed / aria-selected follow the .on class whoever sets it.
(function () {
  const el = SC.el;
  const reduced = () => window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  const NARROW = window.matchMedia('(max-width: 860px)');

  // ------------------------------------------------------------------ rolling digits
  // new SC.Flow(format).el is a span; set(v) shows v. A change rolls each digit to its place,
  // unless changes come faster than SETTLE ms (a playing clip): then the digits snap.
  const SETTLE = 160;
  const DIGIT_EM = 1.2; // the height of one digit in .gd-nf-d (style.css)
  class Flow {
    constructor(format) {
      this.format = format || String;
      this.value = null;
      this.text = '';
      this.last = 0;
      this.cols = [];
      this.sr = el('span', { class: 'sr-only' });
      this.box = el('span', { class: 'gd-nf', 'aria-hidden': 'true' });
      this.el = el('span', {}, this.sr, this.box);
    }
    set(v) {
      if (v === this.value) return;
      const text = this.format(v);
      const now = performance.now();
      const roll = this.value != null && now - this.last > SETTLE && !reduced() && this.el.isConnected;
      const old = this.text;
      this.value = v;
      this.text = text;
      this.last = now;
      this.sr.textContent = text;
      const shape = (s) => s.replace(/\d/g, '0');
      if (shape(text) !== shape(old) || !this.cols.length) this.build(text, roll ? old : null);
      else if (roll) void this.box.offsetWidth; // a node moved into a rebuilt panel lays out its start first
      this.cols.forEach((c, i) => {
        if (!c) return;
        c.parentElement.classList.toggle('gd-snap', !roll);
        c.style.transform = 'translateY(' + -DIGIT_EM * Number(text[i]) + 'em)';
      });
    }
    // New columns for a new shape of the text; they start where the old text, aligned from the
    // right, had its digits, so 9 -> 10 still rolls.
    build(text, from) {
      this.box.textContent = '';
      this.cols = [];
      for (let i = 0; i < text.length; i++) {
        const ch = text[i];
        if (!/\d/.test(ch)) {
          this.box.append(ch);
          this.cols.push(null);
          continue;
        }
        const col = el('span', {}, [...'0123456789'].map((n) => el('span', {}, n)));
        const j = from != null ? from.length - (text.length - i) : -1;
        const start = from != null && j >= 0 && /\d/.test(from[j]) ? Number(from[j]) : 0;
        col.style.transform = 'translateY(' + -DIGIT_EM * start + 'em)';
        this.box.append(el('span', { class: 'gd-nf-d gd-snap' }, col));
        this.cols.push(col);
      }
      void this.box.offsetWidth;
    }
  }
  SC.Flow = Flow;

  // The readout under a 3D window: «тик 14 из 40» with rolling digits and a chip of the phase.
  // set(tick, total, phaseKey, label, more): phaseKey from SC.PHASES colours the chip.
  SC.readout = (box) => {
    const tick = new Flow((v) => SC.num(v));
    const total = el('span');
    const chip = el('span', { class: 'sc-phase' });
    const extra = el('span');
    box.textContent = '';
    box.append(el('span', {}, 'тик ', el('b', {}, tick.el), ' из ', total), chip, extra);
    let lastPhase = null, lastLabel = null, lastExtra = null;
    return {
      set(t, n, phaseKey, label, more) {
        tick.set(t);
        if (total.textContent !== String(n)) total.textContent = n;
        if (phaseKey !== lastPhase || label !== lastLabel) {
          lastPhase = phaseKey;
          lastLabel = label;
          chip.textContent = '';
          if (label) chip.append(el('i', { style: phaseKey ? '--c:' + SC.PHASES[phaseKey].color : '' }), label);
        }
        if (more !== lastExtra) {
          lastExtra = more;
          extra.textContent = more || '';
        }
      },
    };
  };

  // ------------------------------------------------------------------ toasts
  const MAX_TOASTS = 3;
  let toastBox = null;
  const restack = () => [...toastBox.children].filter((x) => !x.classList.contains('gd-out')).reverse()
    .forEach((x, i) => x.style.setProperty('--i', i));
  // SC.toast(title, {desc, tone: 'warn' | 'bad', ms}): newest in front, older ones stacked behind.
  SC.toast = (title, opts) => {
    const o = opts || {};
    if (!toastBox) {
      toastBox = el('ol', { class: 'gd-toasts', 'aria-live': 'polite', 'aria-label': 'Уведомления' });
      document.body.append(toastBox);
    }
    const t = el('li', { class: 'gd-toast' + (o.tone ? ' ' + o.tone : ''), role: o.tone === 'bad' ? 'alert' : 'status' },
      el('i', { 'aria-hidden': 'true' }), el('div', {}, title, o.desc ? el('small', {}, o.desc) : null));
    const close = () => {
      if (t.classList.contains('gd-out')) return;
      t.classList.add('gd-out');
      restack();
      setTimeout(() => t.remove(), reduced() ? 0 : 380);
    };
    t.addEventListener('click', close);
    toastBox.append(t);
    const live = [...toastBox.children].filter((x) => !x.classList.contains('gd-out'));
    if (live.length > MAX_TOASTS) live[0].click();
    restack();
    const ms = o.ms || parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--dur-toast')) || 2600;
    setTimeout(close, ms);
    return close;
  };

  // ------------------------------------------------------------------ links to a moment
  SC.copyLink = (hash, what) => {
    const url = location.href.split('#')[0].split('?')[0] + hash;
    const done = () => SC.toast('Ссылка скопирована', { desc: what || hash });
    const fallback = () => {
      const ta = el('textarea', { style: 'position:fixed;opacity:0' });
      ta.value = url;
      document.body.append(ta);
      ta.select();
      let ok = false;
      try {
        ok = document.execCommand('copy');
      } catch (e) {
        ok = false;
      }
      ta.remove();
      if (ok) done();
      else SC.toast('Не удалось скопировать', { desc: url, tone: 'bad', ms: 6000 });
    };
    if (navigator.clipboard && window.isSecureContext) navigator.clipboard.writeText(url).then(done, fallback);
    else fallback();
  };
  // The page's hash with params replaced: SC.hashWith({clip: 'idle', ct: 3}); null removes one.
  SC.hashWith = (set) => {
    const [h, q] = location.hash.split('?');
    const p = new URLSearchParams(q || '');
    for (const [k, v] of Object.entries(set)) {
      if (v == null) p.delete(k);
      else p.set(k, v);
    }
    const s = p.toString();
    return (h || '#bestiary') + (s ? '?' + s : '');
  };

  // ------------------------------------------------------------------ bottom sheets
  // A Sheet makes a node a modal bottom sheet (.gd-sheet): a grip to drag it down (far or fast
  // enough closes it), the page behind inert, Esc and the scrim close it, focus comes back.
  const CLOSE_PX = 90, FLICK = 0.6;
  const scrim = el('div', { class: 'gd-sheet-scrim' });
  const behind = () => [document.querySelector('.top'), document.getElementById('main'), document.getElementById('err'), document.querySelector('.skip')];
  let openSheet = null;
  class Sheet {
    constructor(node, label, onClose) {
      this.node = node;
      this.label = label;
      this.onClose = onClose;
      this.grip = el('div', { class: 'gd-grip', 'aria-hidden': 'true' });
      let start = null;
      this.grip.addEventListener('pointerdown', (e) => {
        start = { y: e.clientY, t: performance.now() };
        this.grip.setPointerCapture(e.pointerId);
        node.classList.add('gd-dragging');
      });
      this.grip.addEventListener('pointermove', (e) => start && node.style.setProperty('--drag', Math.max(0, e.clientY - start.y) + 'px'));
      const end = (e) => {
        if (!start) return;
        const dy = Math.max(0, e.clientY - start.y), v = dy / Math.max(1, performance.now() - start.t);
        start = null;
        node.classList.remove('gd-dragging');
        if (dy > CLOSE_PX || v > FLICK) this.close();
        else node.style.setProperty('--drag', '0px');
      };
      this.grip.addEventListener('pointerup', end);
      this.grip.addEventListener('pointercancel', end);
      node.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && openSheet === this) {
          e.stopPropagation();
          this.close();
        }
      });
    }
    attach() {
      this.node.classList.add('gd-sheet');
      this.node.setAttribute('role', 'dialog');
      this.node.setAttribute('aria-modal', 'true');
      this.node.setAttribute('aria-label', this.label);
      if (this.grip.parentNode !== this.node) this.node.prepend(this.grip);
    }
    detach() {
      this.close(true);
      this.node.classList.remove('gd-sheet');
      for (const a of ['role', 'aria-modal', 'aria-label']) this.node.removeAttribute(a);
      this.grip.remove();
    }
    open(focus) {
      if (openSheet && openSheet !== this) openSheet.close(true);
      if (!scrim.isConnected) document.body.append(scrim);
      this.back = document.activeElement;
      this.node.style.setProperty('--drag', '0px');
      this.node.classList.add('gd-open');
      scrim.classList.add('gd-open');
      for (const x of behind()) if (x && !x.contains(this.node)) x.inert = true;
      openSheet = this;
      const first = focus || this.node.querySelector('a.on') || this.node.querySelector('button, a, [tabindex]');
      if (first) setTimeout(() => first.focus({ preventScroll: true }), 50);
    }
    close(quiet) {
      if (openSheet !== this) return;
      this.node.classList.remove('gd-open');
      scrim.classList.remove('gd-open');
      for (const x of behind()) if (x) x.inert = false;
      openSheet = null;
      if (this.onClose) this.onClose();
      if (!quiet && this.back && this.back.isConnected) this.back.focus({ preventScroll: true });
    }
  }
  scrim.addEventListener('click', () => openSheet && openSheet.close());
  window.addEventListener('hashchange', () => openSheet && openSheet.close(true));

  // SC.sheet(title, body): details in a sheet from the bottom (the moveset's attacks).
  let detail = null;
  SC.sheet = (title, body) => {
    if (!detail) {
      const node = el('div', {}, el('div', { class: 'gd-sheet-body' }));
      document.body.append(node);
      detail = new Sheet(node, 'Подробности');
      detail.attach();
      node.addEventListener('click', (e) => e.target.closest('.gd-x') && detail.close());
    }
    const x = el('button', { type: 'button', class: 'gd-x' }, 'Закрыть');
    const b = detail.node.querySelector('.gd-sheet-body');
    b.textContent = '';
    b.append(x, el('h3', {}, title), ...[body].flat(Infinity).filter((n) => n != null && n !== false));
    detail.node.setAttribute('aria-label', title);
    b.scrollTop = 0;
    detail.open(x);
    return () => detail.close();
  };

  // The sidebar becomes a sheet on phones, opened by «Разделы» in the top bar. buildNav keeps
  // the grip (app.js), so the nav can be rebuilt while it is a sheet.
  const nav = document.getElementById('nav');
  const navBtn = document.getElementById('nav-open');
  if (nav && navBtn) {
    const navSheet = new Sheet(nav, 'Разделы', () => navBtn.setAttribute('aria-expanded', 'false'));
    navBtn.addEventListener('click', () => {
      navSheet.open();
      navSheet.back = navBtn;
      navBtn.setAttribute('aria-expanded', 'true');
      const on = nav.querySelector('a.on');
      if (on) on.scrollIntoView({ block: 'center' });
    });
    const apply = () => (NARROW.matches ? navSheet.attach() : navSheet.detach());
    NARROW.addEventListener('change', apply);
    apply();
  }

  // The skip link focuses the page instead of changing the hash (the hash is the route).
  const skip = document.querySelector('[data-skip]');
  if (skip)
    skip.addEventListener('click', (e) => {
      e.preventDefault();
      document.getElementById('main').focus();
    });

  // ------------------------------------------------------------------ pointer light on cards
  // .gd-glow cards: the pointer position goes to --mx/--my, style.css draws the light there.
  document.addEventListener('pointermove', (e) => {
    if (e.pointerType !== 'mouse') return;
    const card = e.target.closest && e.target.closest('.gd-glow');
    if (!card || reduced()) return;
    const r = card.getBoundingClientRect();
    card.style.setProperty('--mx', e.clientX - r.left + 'px');
    card.style.setProperty('--my', e.clientY - r.top + 'px');
  }, { passive: true });

  // ------------------------------------------------------------------ sparks and the 3D windows
  // Twelve sparks rise behind the panels. While any 3D window (.sc-stage) is on screen the page
  // is «calm»: the sparks stop and fade, so the viewers keep every frame.
  const SPARKS = 12;
  const sparks = document.getElementById('sparks');
  if (sparks) {
    let seed = 7;
    const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
    for (let i = 0; i < SPARKS; i++) {
      const s = el('i');
      s.style.cssText = `--x:${(rnd() * 100).toFixed(1)}%;--s:${(1.5 + rnd() * 2).toFixed(1)}px;--t:${(9 + rnd() * 9).toFixed(1)}s;` +
        `--d:${(-rnd() * 18).toFixed(1)}s;--dx:${((rnd() - 0.5) * 120).toFixed(0)}px`;
      sparks.append(s);
    }
  }
  const onScreen = new Set();
  const stageWatch = 'IntersectionObserver' in window ? new IntersectionObserver((entries) => {
    for (const e of entries) {
      if (e.isIntersecting && e.target.isConnected) onScreen.add(e.target);
      else onScreen.delete(e.target);
    }
    for (const t of onScreen) if (!t.isConnected) onScreen.delete(t);
    document.body.classList.toggle('calm', onScreen.size > 0);
  }) : null;
  window.addEventListener('hashchange', () => {
    if (stageWatch) stageWatch.disconnect();
    onScreen.clear();
    document.body.classList.remove('calm');
  });

  // ------------------------------------------------------------------ button rows: ARIA and keys
  // A row .sc-clips is a Radix-like toggle group, or tabs when role=tablist. Its scripts only
  // toggle .on; here aria-pressed / aria-selected follow it and the row keeps one tab stop
  // (the pressed button, else the first), arrows move along the row.
  const rowButtons = (row) => [...row.querySelectorAll(':scope > button')];
  function syncRow(row) {
    const bs = rowButtons(row);
    if (!bs.length) return;
    const tabs = row.getAttribute('role') === 'tablist';
    const pick = bs.find((b) => b.classList.contains('on')) || bs[0];
    for (const b of bs) {
      const on = b.classList.contains('on');
      if (tabs) {
        if (b.getAttribute('aria-selected') !== String(on)) b.setAttribute('aria-selected', String(on));
      } else if (b.getAttribute('aria-pressed') !== String(on)) b.setAttribute('aria-pressed', String(on));
      const ti = b === pick ? '0' : '-1';
      if (b.getAttribute('tabindex') !== ti) b.setAttribute('tabindex', ti);
    }
  }
  const pendingRows = new Set();
  let syncQueued = false;
  const observer = new MutationObserver((list) => {
    for (const m of list) {
      const t = m.target;
      const row = t.classList && t.classList.contains('sc-clips') ? t : t.parentElement && t.parentElement.closest('.sc-clips');
      if (row) pendingRows.add(row);
      for (const n of m.addedNodes || []) {
        if (!n.querySelectorAll) continue;
        for (const r of n.querySelectorAll('.sc-clips')) pendingRows.add(r);
        if (stageWatch) {
          if (n.matches('.sc-stage:not(.wip)')) stageWatch.observe(n);
          for (const st of n.querySelectorAll('.sc-stage:not(.wip)')) stageWatch.observe(st);
        }
      }
    }
    if (!syncQueued && pendingRows.size) {
      syncQueued = true;
      queueMicrotask(() => {
        syncQueued = false;
        for (const r of pendingRows) syncRow(r);
        pendingRows.clear();
      });
    }
  });
  observer.observe(document.getElementById('main'), { subtree: true, childList: true, attributes: true, attributeFilter: ['class'] });

  document.addEventListener('keydown', (e) => {
    const b = e.target;
    if (!b || b.tagName !== 'BUTTON' || !b.parentElement || !b.parentElement.classList.contains('sc-clips')) return;
    const keys = { ArrowLeft: -1, ArrowUp: -1, ArrowRight: 1, ArrowDown: 1, Home: -Infinity, End: Infinity };
    if (!(e.key in keys)) return;
    const row = b.parentElement;
    const bs = rowButtons(row);
    const i = bs.indexOf(b), d = keys[e.key];
    const next = d === -Infinity ? bs[0] : d === Infinity ? bs[bs.length - 1] : bs[(i + d + bs.length) % bs.length];
    e.preventDefault();
    for (const x of bs) x.setAttribute('tabindex', x === next ? '0' : '-1');
    next.focus();
    if (row.getAttribute('role') === 'tablist') next.click(); // tabs follow focus, like Radix
  });
})();
