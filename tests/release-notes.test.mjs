import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { execFileSync, spawnSync } from "node:child_process";
import { prepareReleaseNotes } from "../scripts/prepare-release-notes.mjs";

const base = "https://github.com/example/widget/blob/v1.2.3/";
function fixture(run) {
  const root = mkdtempSync(path.join(tmpdir(), "widget-release-notes-"));
  try {
    mkdirSync(path.join(root,"docs"));
    for(const file of ["README.md","docs/report.md","docs/guide (copy).md","docs/release.md"]) writeFileSync(path.join(root,file),"# Fixture\n");
    return run(markdown=>prepareReleaseNotes({markdown,sourcePath:"docs/release.md",root,repository:"example/widget",tag:"v1.2.3"}),root);
  } finally { rmSync(root,{recursive:true,force:true}); }
}

test("release links resolve from the notes directory and preserve queries, anchors and titles",()=>fixture(convert=>{
  const input='[report](report.md#details "Evidence")\r\n[README](../README.md#known-limitations)\r\n[root](/README.md?raw=1#section)';
  assert.equal(convert(input),`[report](${base}docs/report.md#details "Evidence")\r\n[README](${base}README.md#known-limitations)\r\n[root](${base}README.md?raw=1#section)`);
  assert.equal(convert('[copy](<guide (copy).md> "Title")'),`[copy](<${base}docs/guide%20%28copy%29.md> "Title")`);
  assert.equal(convert('[copy](guide%20%28copy%29.md)'),`[copy](${base}docs/guide%20%28copy%29.md)`);
}));

test("external URLs, local headings and code examples stay byte-for-byte intact",()=>fixture(convert=>{
  const unchanged='[web](https://example.org/path?q=1#top) [mail](mailto:example@example.org) [cdn](//example.org/file) [heading](#section)\n`[inline](missing.md)`\n``[inline ` code](missing.md)``\n```markdown\n[example](missing.md)\n[ref]: missing.md\n```\n~~~\n[example](missing.md)\n~~~~\n';
  assert.equal(convert(unchanged),unchanged);
  assert.equal(convert(unchanged+'[real](report.md)'),unchanged+`[real](${base}docs/report.md)`);
}));

test("nested labels, escaped delimiters and reference definitions retain Markdown structure",()=>fixture(convert=>{
  assert.equal(convert('[a [nested] label](report.md)'),`[a [nested] label](${base}docs/report.md)`);
  assert.equal(convert('[copy](guide%20\\(copy\\).md)'),`[copy](${base}docs/guide%20%28copy%29.md)`);
  assert.equal(convert('\\[literal](missing.md)'), '\\[literal](missing.md)');
  assert.equal(convert('[report][ref]\n[ref]: report.md#details "Title"\n[with:colon]: <guide (copy).md>'),`[report][ref]\n[ref]: ${base}docs/report.md#details "Title"\n[with:colon]: <${base}docs/guide%20%28copy%29.md>`);
}));

test("missing files and paths escaping the repository fail before publication",()=>fixture(convert=>{
  assert.throws(()=>convert('[missing](missing.md)'),/not a file/);
  assert.throws(()=>convert('[outside](../../README.md)'),/escapes the repository/);
  assert.throws(()=>convert('[outside](%2e%2e/%2e%2e/README.md)'),/escapes the repository/);
  assert.throws(()=>convert('[directory](../docs)'),/not a file/);
}));

test("the actual v0.1.5 notes generate seven tag-pinned links without source edits",()=>{
  const sourcePath="docs/v0.1.5-release-notes.md",markdown=readFileSync(sourcePath,"utf8");
  const output=prepareReleaseNotes({markdown,sourcePath,repository:"andali89/codex-usage-widget-app",tag:"v0.1.5"});
  const links=[...output.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)].map(m=>m[1]);
  assert.equal(links.length,7);
  assert.ok(links.every(url=>url.startsWith("https://github.com/andali89/codex-usage-widget-app/blob/v0.1.5/")));
  assert.ok(links.includes("https://github.com/andali89/codex-usage-widget-app/blob/v0.1.5/README.md#known-limitations"));
  assert.ok(links.some(url=>url.endsWith("/docs/account-update-empty-quota-investigation.md#correction-validation--2026-11-04")));
  assert.equal(readFileSync(sourcePath,"utf8"),markdown);
});

test("the CLI writes a separate file and refuses overwriting its source",()=>fixture((_,root)=>{
  const input=path.join(root,"docs/release.md"),output=path.join(root,"generated/notes.md");
  writeFileSync(input,'[report](report.md)');
  const script=path.resolve("scripts/prepare-release-notes.mjs");
  const args=[script,"--input","docs/release.md","--repository","example/widget","--tag","v1.2.3"];
  execFileSync(process.execPath,[...args,"--output",output],{cwd:root,windowsHide:true});
  assert.equal(readFileSync(output,"utf8"),`[report](${base}docs/report.md)`);
  assert.equal(readFileSync(input,"utf8"),'[report](report.md)');
  const refused=spawnSync(process.execPath,[...args,"--output",input],{cwd:root,encoding:"utf8",windowsHide:true});
  assert.equal(refused.status,1);assert.match(refused.stderr,/must not overwrite/);
  if(process.platform==="win32"){
    const differentCase=spawnSync(process.execPath,[...args,"--output",input.toUpperCase()],{cwd:root,encoding:"utf8",windowsHide:true});
    assert.equal(differentCase.status,1);assert.match(differentCase.stderr,/must not overwrite/);
  }
  writeFileSync(input,'[missing](missing.md)');
  const missing=path.join(root,"missing-output.md");
  assert.equal(spawnSync(process.execPath,[...args,"--output",missing],{cwd:root,windowsHide:true}).status,1);
  assert.equal(existsSync(missing),false);
}));

test("the workflow prepares notes before building and passes only the generated file to gh",()=>{
  const workflow=readFileSync(".github/workflows/release.yml","utf8");
  assert.ok(workflow.indexOf("name: Set up Node.js")<workflow.indexOf("name: Prepare tag-pinned release notes"));
  assert.ok(workflow.indexOf("name: Prepare tag-pinned release notes")<workflow.indexOf("name: Set up Rust"));
  assert.match(workflow,/RELEASE_NOTES_FILE: \$\{\{ steps\.notes\.outputs\.path \}\}/);
  assert.match(workflow,/--notes-file \$env:RELEASE_NOTES_FILE/);
  assert.doesNotMatch(workflow,/--notes-file "docs\//);
});
