// Generate with `cd ../ && find dist -type f`
var filesToCache = [
  "/",
  "/index.html",
  "/pkg/aldonlib_bg.wasm",
  "/pkg/package.json",
  "/pkg/aldonlib.js",
  "/assets/icon.png",
  "/assets/palmos.ttf",
  "/assets/palmos_bold.ttf",
  "/assets/qr.svg",
  "/assets/spritesheet.png",
  "/dialog.js",
  "/index.html",
  "/main.js",
  "/manifest.json",
];

/* Start the service worker and cache all of the app's content */
self.addEventListener("install", function (e) {
  // TODO: haven't gotten this to work yet
  /*
  e.waitUntil(
    caches.open("aldon").then(function (cache) {
      return cache.addAll(filesToCache);
    }),
  );
  self.skipWaiting();
  */
});

/* Serve cached content when offline */
self.addEventListener("fetch", function (e) {
  e.respondWith(
    caches.match(e.request).then(function (response) {
      return response || fetch(e.request);
    }),
  );
});
