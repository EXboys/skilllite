/**
 * Fail if production JS still contains `(?<=` — used by mdast-util-gfm-autolink-literal
 * and rejected by older WKWebView (Tauri) with "invalid group specifier name".
 */
import { readdirSync, readFileSync, existsSync } from "node:fs";
import { join } from "node:path";

const dir = join(process.cwd(), "dist", "assets");
if (!existsSync(dir)) {
  console.error("verify-no-lookbehind: missing dist/assets (run npm run build first)");
  process.exit(1);
}

const re = /\(\?<=/;
const files = readdirSync(dir).filter((f) => f.endsWith(".js"));
if (files.length === 0) {
  console.error("verify-no-lookbehind: no .js under dist/assets");
  process.exit(1);
}

for (const f of files) {
  const p = join(dir, f);
  const s = readFileSync(p, "utf8");
  if (re.test(s)) {
    console.error(`verify-no-lookbehind: FAIL — found (?<= in ${p}`);
    process.exit(1);
  }
}

console.log("verify-no-lookbehind: OK (no (?<= in dist/assets/*.js)");
