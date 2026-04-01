const CACHE_NAME = 'dnd-pc-v2';

const BASE = new URL('./', self.location.href).href;
const LOCALES = ['en', 'ru'];

const FIREBASE_URLS = [
  'https://www.gstatic.com/firebasejs/11.4.0/firebase-app.js',
  'https://www.gstatic.com/firebasejs/11.4.0/firebase-auth.js',
  'https://www.gstatic.com/firebasejs/11.4.0/firebase-firestore.js',
];

// Build data file URLs from index.json
async function buildPrecacheList() {
  const urls = [...FIREBASE_URLS];
  try {
    const resp = await fetch(new URL('data/index.json', BASE));
    const index = await resp.json();

    // Top-level data files
    urls.push(
      new URL('data/index.json', BASE).href,
      new URL('data/features.json', BASE).href,
      new URL('data/effects.json', BASE).href,
      new URL('data/names.json', BASE).href,
    );

    // All entries from index categories (classes, species, backgrounds, spells)
    const entryUrls = [];
    for (const category of Object.values(index)) {
      for (const entry of category) {
        if (entry.url) entryUrls.push(entry.url);
      }
    }

    // Data files + locale overlays
    for (const url of entryUrls) {
      urls.push(new URL(`data/${url}`, BASE).href);
      for (const locale of LOCALES) {
        urls.push(new URL(`${locale}/${url}`, BASE).href);
      }
    }

    // Locale top-level files
    for (const locale of LOCALES) {
      urls.push(
        new URL(`${locale}/index.json`, BASE).href,
        new URL(`${locale}/features.json`, BASE).href,
        new URL(`${locale}/effects.json`, BASE).href,
      );
    }
  } catch (e) {
    // If index fetch fails, just precache Firebase URLs
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

self.addEventListener('fetch', (event) => {
  // Skip non-GET requests
  if (event.request.method !== 'GET') return;

  const isSameOrigin = event.request.url.startsWith(self.location.origin);
  const isPrecached = FIREBASE_URLS.some((url) => event.request.url.startsWith(url));

  // Only handle same-origin and precached cross-origin requests
  if (!isSameOrigin && !isPrecached) return;

  event.respondWith(
    caches.match(event.request).then((cached) => {
      if (cached) {
        // Return cached, but also update cache in background
        fetch(event.request)
          .then((response) => {
            if (response.ok) {
              const clone = response.clone();
              caches.open(CACHE_NAME).then((cache) => cache.put(event.request, clone));
            }
          })
          .catch(() => {});
        return cached;
      }

      return fetch(event.request).then((response) => {
        if (response.ok) {
          const clone = response.clone();
          caches.open(CACHE_NAME).then((cache) => cache.put(event.request, clone));
        }
        return response;
      });
    })
  );
});
