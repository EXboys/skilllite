const assert = require("node:assert/strict");
const path = require("node:path");
const fs = require("node:fs");
const vm = require("node:vm");
const test = require("node:test");
const ts = require("typescript");

const workspaceRoot = path.resolve(__dirname, "..");
const srcRoot = path.join(workspaceRoot, "src");

function loadTsModule(entryPath, cache = new Map()) {
  const fullPath = entryPath.endsWith(".ts") ? entryPath : `${entryPath}.ts`;
  if (cache.has(fullPath)) return cache.get(fullPath).exports;

  const source = fs.readFileSync(fullPath, "utf8");
  const transpiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2020,
      esModuleInterop: true,
    },
    fileName: fullPath,
  }).outputText;

  const module = { exports: {} };
  cache.set(fullPath, module);

  function localRequire(specifier) {
    if (specifier.startsWith(".")) {
      return loadTsModule(path.resolve(path.dirname(fullPath), specifier), cache);
    }
    return require(specifier);
  }

  const context = {
    module,
    exports: module.exports,
    require: localRequire,
    __dirname: path.dirname(fullPath),
    __filename: fullPath,
    console,
    process,
    setTimeout,
    clearTimeout,
  };
  vm.runInNewContext(transpiled, context, { filename: fullPath });
  return module.exports;
}

const fallbackMod = loadTsModule(path.join(srcRoot, "utils", "llmScenarioFallback"));

const {
  buildScenarioCandidates,
  runWithScenarioFallback,
  resetLlmFallbackCooldown,
  unwrapStructuredLlmInvokeResult,
} = fallbackMod;

function makeSettings() {
  return {
    provider: "api",
    apiKey: "main-key",
    model: "gpt-4o",
    workspace: ".",
    apiBase: "https://api.openai.com/v1",
    sandboxLevel: 3,
    swarmEnabled: false,
    swarmUrl: "",
    locale: "zh",
    llmProfiles: [
      {
        id: "p1",
        provider: "api",
        model: "gpt-4o",
        apiBase: "https://api.openai.com/v1",
        apiKey: "k1",
      },
      {
        id: "p2",
        provider: "api",
        model: "gpt-4o-mini",
        apiBase: "https://api.openai.com/v1",
        apiKey: "k2",
      },
      {
        id: "p3",
        provider: "api",
        model: "claude-sonnet",
        apiBase: "https://api.anthropic.com/v1",
        apiKey: "k3",
      },
    ],
    llmScenarioRoutingEnabled: true,
    llmScenarioRoutes: { followup: "p1" },
    llmScenarioFallbacks: { followup: ["p1", "missing", "p2", "p2", "p3"] },
  };
}

test("buildScenarioCandidates filters missing and duplicate profile ids", () => {
  const settings = makeSettings();
  const out = buildScenarioCandidates(settings, "followup");
  assert.equal(out.length, 3);
  assert.equal(
    JSON.stringify(out.map((x) => ({ id: x.profileId, primary: x.isPrimary }))),
    JSON.stringify([
      { id: "p1", primary: true },
      { id: "p2", primary: false },
      { id: "p3", primary: false },
    ])
  );
});

test("runWithScenarioFallback switches to next fallback on retryable structured error", async () => {
  resetLlmFallbackCooldown();
  const settings = makeSettings();
  const used = [];
  const result = await runWithScenarioFallback(
    settings,
    "followup",
    async (_config, attempt) => {
      used.push(attempt.profileId);
      if (attempt.profileId === "p1") {
        return unwrapStructuredLlmInvokeResult({
          ok: false,
          error: {
            kind: "rate_limited",
            retryable: true,
            message: "HTTP 429",
          },
        });
      }
      return `ok:${attempt.profileId}`;
    },
    { cooldownMs: 5_000 }
  );
  assert.deepEqual(used, ["p1", "p2"]);
  assert.equal(result.result, "ok:p2");
  assert.equal(result.usedProfileId, "p2");
  assert.equal(result.switched, true);
});

test("runWithScenarioFallback does not switch on non-retryable structured error", async () => {
  resetLlmFallbackCooldown();
  const settings = makeSettings();
  const used = [];
  await assert.rejects(
    runWithScenarioFallback(settings, "followup", async (_config, attempt) => {
      used.push(attempt.profileId);
      return unwrapStructuredLlmInvokeResult({
        ok: false,
        error: {
          kind: "missing_api_key",
          retryable: false,
          message: "missing key",
        },
      });
    }),
    /missing key/
  );
  assert.deepEqual(used, ["p1"]);
});

test("runWithScenarioFallback skips a profile that is cooling down", async () => {
  resetLlmFallbackCooldown();
  const settings = makeSettings();
  const firstUsed = [];
  await runWithScenarioFallback(
    settings,
    "followup",
    async (_config, attempt) => {
      firstUsed.push(attempt.profileId);
      if (attempt.profileId === "p1") {
        throw Object.assign(new Error("network timeout"), {
          retryable: true,
        });
      }
      return `ok:${attempt.profileId}`;
    },
    { cooldownMs: 60_000 }
  );
  assert.deepEqual(firstUsed, ["p1", "p2"]);

  const secondUsed = [];
  const second = await runWithScenarioFallback(
    settings,
    "followup",
    async (_config, attempt) => {
      secondUsed.push(attempt.profileId);
      return `ok:${attempt.profileId}`;
    },
    { cooldownMs: 60_000 }
  );
  assert.deepEqual(secondUsed, ["p2"]);
  assert.equal(second.usedProfileId, "p2");
});
