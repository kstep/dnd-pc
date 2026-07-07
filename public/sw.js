// __BUILD_HASH__ is replaced with the git SHA by .github/workflows/deploy.yml
// before `trunk build`, so each release ships a byte-different sw.js — that's
// what triggers the browser to install a new worker.
const CACHE_NAME = 'dnd-pc-__BUILD_HASH__';

const BASE = new URL('./', self.location.href).href;
const LOCALES = ['en', 'ru'];

const FIREBASE_URLS = [
  'https://www.gstatic.com/firebasejs/11.4.0/firebase-app.js',
  'https://www.gstatic.com/firebasejs/11.4.0/firebase-auth.js',
  'https://www.gstatic.com/firebasejs/11.4.0/firebase-firestore.js',
];

// Fixed per-package rules files; packages that don't ship one 404 harmlessly
// (per-URL cache.add failures are tolerated below).
const PKG_FILES = ['classes.json', 'species.json', 'backgrounds.json',
                   'features.json', 'spells.json', 'effects.json'];

// Build data file URLs from the package manifest (rules/index.json)
async function buildPrecacheList() {
  const urls = [
    ...FIREBASE_URLS,
    new URL('rules/names.json', BASE).href,
    new URL('rules/index.json', BASE).href,
  ];
  try {
    const manifest = await (await fetch(new URL('rules/index.json', BASE))).json();
    for (const pkg of manifest) {
      for (const file of PKG_FILES) {
        urls.push(new URL(`rules/${pkg.id}/data/${file}`, BASE).href);
        for (const locale of LOCALES) {
          urls.push(new URL(`rules/${pkg.id}/${locale}/${file}`, BASE).href);
        }
      }
      // Spell lists have variable names; every "spells/<x>.json" Ref path
      // appears verbatim in the package's features.json text.
      try {
        const features = await (
          await fetch(new URL(`rules/${pkg.id}/data/features.json`, BASE))
        ).text();
        for (const match of features.matchAll(/"(spells\/[^"]+\.json)"/g)) {
          urls.push(new URL(`rules/${pkg.id}/data/${match[1]}`, BASE).href);
        }
      } catch (e) {}
    }
  } catch (e) {
    // If the manifest fetch fails, just precache Firebase URLs
  }
  return urls;
}

self.addEventListener('install', (event) => {
  event.waitUntil(
    buildPrecacheList().then((urls) =>
      caches.open(CACHE_NAME).then((cache) =>
        // Use individual fetches so one 404 doesn't abort the whole precache
        Promise.all(
          urls.map((url) =>
            cache.add(url).catch(() => {})
          )
        )
      )
    )
  );
  self.skipWaiting();
});

// The waiting worker stays parked until the page asks it to take over.
self.addEventListener('message', (event) => {
  if (event.data && event.data.type === 'SKIP_WAITING') self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((names) =>
      Promise.all(
        names
          .filter((name) => name !== CACHE_NAME)
          .map((name) => caches.delete(name))
      )
    )
  );
  self.clients.claim();
});

// Trunk-emitted bundles carry a content hash in the filename, so the URL is
// already immutable — caching them in the SW only opens a window for the cache
// to disagree with the SRI digest in index.html (e.g. when a 404 falls through
// to the SPA index.html and gets stored under a JS URL).
const HASHED_ASSET_RE = /-[a-f0-9]{8,}\.(?:js|wasm|css|map)(?:\?|$)/;

function isCacheable(response) {
  return (
    response.ok &&
    response.type !== 'opaque' &&
    response.type !== 'opaqueredirect'
  );
}

self.addEventListener('fetch', (event) => {
  if (event.request.method !== 'GET') return;

  const url = new URL(event.request.url);
  const isSameOrigin = url.origin === self.location.origin;
  const isPrecached = FIREBASE_URLS.some((u) => event.request.url.startsWith(u));

  if (!isSameOrigin && !isPrecached) return;

  if (isSameOrigin && HASHED_ASSET_RE.test(url.pathname)) return;

  const isNavigation =
    event.request.mode === 'navigate' ||
    event.request.destination === 'document';

  if (isNavigation) {
    event.respondWith(
      fetch(event.request)
        .then((response) => {
          if (isCacheable(response) && response.type === 'basic') {
            const clone = response.clone();
            caches.open(CACHE_NAME).then((cache) => cache.put(event.request, clone));
          }
          return response;
        })
        .catch(() =>
          caches.match(event.request).then((cached) => cached || caches.match(BASE))
        )
    );
    return;
  }

  event.respondWith(
    caches.match(event.request).then((cached) => {
      const networkUpdate = fetch(event.request)
        .then((response) => {
          if (isCacheable(response)) {
            const clone = response.clone();
            caches.open(CACHE_NAME).then((cache) => cache.put(event.request, clone));
          }
          return response;
        })
        .catch(() => null);

      if (cached) {
        networkUpdate.catch(() => {});
        return cached;
      }
      return networkUpdate.then((response) => response || Response.error());
    })
  );
});
