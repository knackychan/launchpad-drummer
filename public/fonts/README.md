The sandbox could not download the required OFL font assets. The CSS already references these local files; until they are supplied, it uses Bahnschrift / Segoe UI and Consolas fallbacks. Do not treat the browser screenshots as typography approval.

Before the visual acceptance pass, bundle:

- `antonio-latin-wght-normal.woff2`: Antonio variable, normal, weights 100–700.
- `share-tech-mono-latin-400-normal.woff2`: Share Tech Mono, normal, weight 400.
- Each family's OFL license and copyright notice.

Sources: [Antonio](https://fontsource.org/fonts/antonio) and [Share Tech Mono](https://fontsource.org/fonts/share-tech-mono). Copy the binary files into this folder, without adding runtime network requests. Their font names, weights and filenames must match `src/styles/tokens.css`.
