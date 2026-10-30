// Packs the site into one HTML file that works offline, opened straight from disk (file://): styles,
// fonts, images, scripts and the WebAssembly engine are inlined. Browsers do not let a page opened from
// disk fetch the .wasm file next to it, so the engine travels as base64 (window.DAMA_WASM_B64).
// usage: node site/tools/bundle.js [output]   (default site/dist/dama.html)
'use strict';
const fs = require('fs');
const path = require('path');

const site = path.resolve(__dirname, '..');
const out = path.resolve(process.argv[2] || path.join(site, 'dist', 'dama.html'));
const file = (p) => fs.readFileSync(path.join(site, p));
const MIME = { '.woff2': 'font/woff2', '.png': 'image/png', '.svg': 'image/svg+xml', '.jpg': 'image/jpeg' };
const dataUrl = (p) => {
  const type = MIME[path.extname(p)];
  if (!type) throw new Error(`no media type for ${p}`);
  return `data:${type};base64,${file(p).toString('base64')}`;
};
const used = [];

let html = file('index.html').toString('utf8');

// Stylesheets, with their url(...) references inlined relative to the stylesheet.
html = html.replace(/<link rel="stylesheet" href="([^"]+)">/g, (_, href) => {
  used.push(href);
  const dir = path.dirname(href);
  const css = file(href).toString('utf8').replace(/url\((['"]?)([^'")]+)\1\)/g, (m, q, u) => {
    if (u.startsWith('data:')) return m;
    const p = path.join(dir, u);
    used.push(p);
    return `url(${dataUrl(p)})`;
  });
  return `<style>\n${css}\n</style>`;
});

// Images.
html = html.replace(/<img([^>]*?) src="([^"]+)"/g, (_, attrs, src) => {
  used.push(src);
  return `<img${attrs} src="${dataUrl(src)}"`;
});

// Scripts, in place and in order. "</script" inside a script would end the tag early.
html = html.replace(/<script src="([^"]+)"><\/script>/g, (_, src) => {
  used.push(src);
  let js = file(src).toString('utf8');
  if (src === 'pkg/dama_web.js') {
    used.push('pkg/dama_web_bg.wasm');
    js += `\nwindow.DAMA_WASM_B64 = "${file('pkg/dama_web_bg.wasm').toString('base64')}";\n`;
  }
  return `<script>\n${js.replace(/<\/script/gi, '<\\/script')}\n</script>`;
});

if (/(?:src|href)="(?!data:|#|https?:)[^"]+"/.test(html.replace(/<script>[\s\S]*?<\/script>/g, ''))) {
  throw new Error('a local reference was left in the page: add its kind to bundle.js');
}
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, html);
console.log(`wrote ${path.relative(process.cwd(), out)} (${(html.length / 1024).toFixed(0)} KB) from ${used.length} files:`);
for (const u of used) console.log(`  ${u}`);
