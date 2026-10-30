// Render the dossier HTML to an A4 PDF with the preinstalled Playwright Chromium.
// Usage: node render.mjs <in.html> <out.pdf>
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";
const require = createRequire(import.meta.url);
const { chromium } = require("playwright");
const [, , input, output] = process.argv;
const browser = await chromium.launch();
const page = await browser.newPage();
await page.emulateMedia({ media: "print", colorScheme: "light" });
await page.goto(pathToFileURL(input).href, { waitUntil: "load" });
await page.evaluate(() => document.fonts.ready);
await page.pdf({ path: output, format: "A4", printBackground: true, preferCSSPageSize: true, outline: true, tagged: true });
await browser.close();
