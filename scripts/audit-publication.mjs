// Read-only heuristic audit. Never prints matching values or changes Git history.
// Review the reported locations: test fixtures and schema fields can be benign.
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";

function git(args, input) {
  const result = spawnSync("git", args, {
    input,
    maxBuffer: 256 * 1024 * 1024,
    windowsHide: true,
  });
  if (result.error || result.status !== 0) {
    throw new Error(`Git audit command failed: ${args[0]}`);
  }
  return result.stdout;
}

const rules = [
  ["private-key", /-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----/g],
  ["service-token", /\b(?:sk-(?:proj-|svcacct-)?[A-Za-z0-9_-]{20,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{30,}|AKIA[A-Z0-9]{16}|ASIA[A-Z0-9]{16}|AIza[A-Za-z0-9_-]{30,})\b/g],
  ["jwt", /\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b/g],
  ["credential-assignment", /["']?(?:api[_-]?key|access[_-]?token|refresh[_-]?token|client[_-]?secret|password|authorization)["']?\s*[:=]\s*["'][^"'\r\n]{8,}["']/gi],
  ["url-userinfo", /\bhttps?:\/\/[^\s/<>"']+:[^\s/<>"']+@/gi],
  ["email-address", /\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b/g],
  ["user-home-path", /(?:[A-Za-z]:[\\/]+Users[\\/]+[^\s\\/<>"'`]+|\/(?:Users|home)\/[^\s/<>"'`]+)/gi],
];

function scan(text) {
  const findings = [];
  for (const [category, pattern] of rules) {
    pattern.lastIndex = 0;
    for (const match of text.matchAll(pattern)) {
      findings.push({
        category,
        line: 1 + text.slice(0, match.index).split("\n").length - 1,
      });
    }
  }
  return findings;
}

function pathRisk(path) {
  if (/(?:^|\/)(?:node_modules|dist|target|gen)\//i.test(path)) return "generated-output";
  if (/(?:^|\/)(?:\.env(?:\..*)?|auth\.json|credentials(?:\.json)?|\.git-credentials|\.npmrc|id_rsa|id_ed25519|state(?:\.backup)?\.json|state\.lock|storage-location(?:\.backup)?\.json)$/i.test(path) && !path.endsWith(".env.example")) return "local-credentials-or-state";
  if (/(?:^|\/)(?:\.codex|\.agents|\.aws|storage-locations)\//i.test(path)) return "local-private-directory";
  if (/\.(?:pfx|p12|pem|key|exe|dll|pdb|log|zip|7z)$/i.test(path)) return "private-material-or-build-artifact";
  if (/(?:codex-widget-design-board|Modern Dual Progress Dashboard|HUD-segemented reset|Edge-segemented reset)/i.test(path)) return "private-reference-asset";
  return null;
}

if (process.argv.includes("--self-test")) {
  for (const [value, category] of [
    ["sk-" + "a".repeat(30), "service-token"],
    ["ghp_" + "a".repeat(30), "service-token"],
    ["AKIA" + "A".repeat(16), "service-token"],
    ["-----BEGIN " + "PRIVATE KEY-----", "private-key"],
    ["eyJ" + "a".repeat(12) + "." + "b".repeat(12) + "." + "c".repeat(12), "jwt"],
    ['api_key = "' + "example-value" + '"', "credential-assignment"],
    ["https://" + "example:example@host", "url-userinfo"],
  ]) assert(scan(value).some(finding => finding.category === category), category);
  assert.deepEqual(scan('{"accessToken":{"type":"string"}}'), []);
  assert.equal(pathRisk("schemas/codex/0.163.0/v2/GetAccountResponse.json"), null);
  assert.equal(pathRisk("src-tauri/target/release/app.exe"), "generated-output");
  assert.equal(pathRisk(".codex/auth.json"), "local-credentials-or-state");
  console.log("Publication audit self-checks passed; matching values were not printed.");
  process.exit(0);
}

const tracked = git(["ls-files", "-z"]).toString("utf8").split("\0").filter(Boolean);
const historicalPaths = [...new Set(git(["log", "--all", "--format=", "--name-only", "-z"])
  .toString("utf8").split(/[\0\n]/).filter(Boolean))];
const workingFindings = [];
for (const path of tracked) {
  const buffer = readFileSync(path);
  if (!buffer.includes(0)) {
    for (const finding of scan(buffer.toString("utf8"))) workingFindings.push({ path, ...finding });
  }
}

const objects = git(["rev-list", "--objects", "--all"]).toString("utf8").trim().split("\n").filter(Boolean);
const paths = new Map(objects.map(line => {
  const space = line.indexOf(" ");
  return space < 0 ? [line, "<commit-or-tree>"] : [line.slice(0, space), line.slice(space + 1)];
}));
const batch = git(["cat-file", "--batch"], objects.map(line => line.split(" ", 1)[0]).join("\n") + "\n");
const historicalFindings = [];
let offset = 0;
let blobs = 0;
let textBlobs = 0;
let binaryBlobs = 0;
const binaryBlobPaths = new Set();
let commitObjects = 0;
while (offset < batch.length) {
  const end = batch.indexOf(10, offset);
  const [id, type, sizeText] = batch.subarray(offset, end).toString("ascii").split(" ");
  const size = Number(sizeText);
  if (end < 0 || !Number.isSafeInteger(size) || size < 0 || end + 1 + size >= batch.length) {
    throw new Error("Invalid Git object batch");
  }
  const bytes = batch.subarray(end + 1, end + 1 + size);
  offset = end + 1 + size + 1;
  if (type === "commit") {
    commitObjects++;
    // Names/emails are reported separately; scan commit messages for token-like data.
    const message = bytes.toString("utf8").split("\n\n").slice(1).join("\n\n");
    for (const finding of scan(message)) historicalFindings.push({ object: id, path: "<commit-message>", ...finding });
  } else if (type === "blob") {
    blobs++;
    if (bytes.includes(0)) {
      binaryBlobs++;
      binaryBlobPaths.add(paths.get(id));
      continue;
    }
    textBlobs++;
    for (const finding of scan(bytes.toString("utf8"))) historicalFindings.push({ object: id, path: paths.get(id), ...finding });
  }
}

const emails = [...new Set(git(["log", "--all", "--format=%ae%n%ce"])
  .toString("utf8").trim().split("\n").filter(Boolean))];
const refs = git(["for-each-ref", "--format=%(refname)"]).toString("utf8").trim().split("\n").filter(Boolean);
const privateReferences = [
  "docs/assets/codex-widget-design-board.png",
  "docs/assets/Modern Dual Progress Dashboard.png",
  "docs/assets/HUD-segemented reset.png",
  "docs/assets/Edge-segemented reset.png",
];
const ignored = spawnSync("git", ["check-ignore", "--", ...privateReferences], { windowsHide: true });
if (ignored.error || ![0, 1].includes(ignored.status)) throw new Error("Ignore check failed");
const ignoredPaths = ignored.stdout.toString("utf8").trim().split("\n");

console.log(JSON.stringify({
  auditedHead: git(["rev-parse", "HEAD"]).toString("ascii").trim(),
  trackedFiles: tracked.length,
  reachableRefs: refs,
  commitObjects,
  historicalBlobs: blobs,
  textBlobs,
  binaryBlobs,
  binaryBlobPaths: [...binaryBlobPaths],
  currentPathRisks: tracked.flatMap(path => pathRisk(path) ? [{ path, category: pathRisk(path) }] : []),
  historicalPathRisks: historicalPaths.flatMap(path => pathRisk(path) ? [{ path, category: pathRisk(path) }] : []),
  privateReferencesIgnored: privateReferences.every(path => ignoredPaths.includes(path)),
  gitIdentity: {
    uniqueEmailAddresses: emails.length,
    allGitHubNoreply: emails.every(email => /@users\.noreply\.github\.com$/i.test(email)),
  },
  workingFindings,
  historicalFindings,
  limits: "Heuristic text/path scan of tracked working files and objects reachable from all refs; review reported locations. Binary content, unreachable objects, external resources, and untracked files are not secret-scanned. Matching values and Git email addresses are never printed.",
}, null, 2));
