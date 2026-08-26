const SHELL_CACHE = 'perilous-app-shell-v1';
const APP_SHELL = [
  '/',
  '/manifest.json',
  '/favicon.png?v=20260822-stockade-light',
  '/perilouslogo-180x180.png?v=20260822-stockade-light',
];
const RUNTIME_ASSETS = new Set([
  '/sp2.desktop.js',
  '/sp2.mobile.js',
  '/manifest.json',
  '/favicon.png',
  '/perilouslogo-180x180.png',
]);

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches
      .open(SHELL_CACHE)
      .then((cache) => cache.addAll(APP_SHELL))
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(
        keys
          .filter((key) => key.startsWith('perilous-app-shell-') && key !== SHELL_CACHE)
          .map((key) => caches.delete(key)),
      ))
      .then(() => self.clients.claim()),
  );
});

async function networkFirst(request, fallbackUrl) {
  const cache = await caches.open(SHELL_CACHE);

  try {
    const response = await fetch(request);
    if (response.ok) {
      await cache.put(request, response.clone());
    }
    return response;
  } catch (error) {
    const cached = await cache.match(request, { ignoreSearch: true });
    if (cached) return cached;

    if (fallbackUrl) {
      const fallback = await cache.match(fallbackUrl, { ignoreSearch: true });
      if (fallback) return fallback;
    }

    throw error;
  }
}

self.addEventListener('fetch', (event) => {
  const request = event.request;
  if (request.method !== 'GET') return;

  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;

  if (request.mode === 'navigate') {
    if (url.pathname === '/' && url.search === '') {
      event.respondWith(networkFirst(request, '/'));
    }
    return;
  }

  if (RUNTIME_ASSETS.has(url.pathname)) {
    event.respondWith(networkFirst(request));
  }
});
