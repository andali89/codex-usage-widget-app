import { mkdirSync, readFileSync, realpathSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { parseArgs } from "node:util";

const escaped = (text, index) => {
  let count = 0;
  while (index > 0 && text[--index] === "\\") count++;
  return count % 2 === 1;
};

// Preserve offsets while excluding fenced code and inline code from link parsing.
function proseOnly(markdown) {
  const chars = markdown.split("");
  const mask = (start, end) => {
    for (let i = start; i < end; i++) if (chars[i] !== "\n" && chars[i] !== "\r") chars[i] = " ";
  };
  let fence = null;
  let offset = 0;
  for (const line of markdown.split(/(?<=\n)/)) {
    const marker = /^ {0,3}(`{3,}|~{3,})(.*)/.exec(line);
    if (fence) {
      mask(offset, offset + line.length);
      if (marker && marker[1][0] === fence[0] && marker[1].length >= fence.length && !marker[2].trim()) fence = null;
    } else if (marker) {
      fence = marker[1];
      mask(offset, offset + line.length);
    }
    offset += line.length;
  }
  for (let i = 0; i < chars.length; i++) {
    if (chars[i] !== "`" || escaped(markdown, i)) continue;
    let end = i;
    while (chars[end] === "`") end++;
    const delimiter = "`".repeat(end - i);
    const prose = chars.join("");
    let closing = prose.indexOf(delimiter, end);
    while (closing !== -1 && (prose[closing - 1] === "`" || prose[closing + delimiter.length] === "`")) closing = prose.indexOf(delimiter, closing + delimiter.length);
    if (closing !== -1) {
      mask(i, closing + delimiter.length);
      i = closing + delimiter.length - 1;
    } else i = end - 1;
  }
  return chars.join("");
}

// Read only the destination, leaving labels, optional titles and formatting intact.
function inlineDestination(prose, opening) {
  let i = opening + 1;
  while (/\s/.test(prose[i] ?? "") && i < prose.length) i++;
  const angled = prose[i] === "<";
  const start = angled ? ++i : i;
  let depth = 0;
  while (i < prose.length) {
    const c = prose[i];
    if (c === "\\") { i += 2; continue; }
    if (angled ? c === ">" : (c === ")" && depth === 0) || /\s/.test(c)) break;
    if (!angled && c === "(") depth++;
    if (!angled && c === ")") depth--;
    i++;
  }
  const end = i;
  if (angled && prose[i++] !== ">") return null;
  const gap = i;
  while (i < prose.length && /\s/.test(prose[i])) i++;
  if (i > gap && ["\"", "'", "("].includes(prose[i])) {
    const closing = prose[i] === "(" ? ")" : prose[i];
    i++;
    while (i < prose.length && (prose[i] !== closing || escaped(prose, i))) i++;
    i++;
    while (i < prose.length && /\s/.test(prose[i])) i++;
  }
  return prose[i] === ")" ? { start, end, closing: i } : null;
}

function insideRoot(root, candidate) {
  const relative = path.relative(root, candidate);
  if (relative === ".." || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) throw new Error("Release-note path escapes the repository.");
  return relative.split(path.sep).join("/");
}

const encodeSegment = segment => encodeURIComponent(segment).replace(/[!'()*]/g, char => `%${char.charCodeAt(0).toString(16).toUpperCase()}`);

export function prepareReleaseNotes({ markdown, sourcePath, repository, tag, root = process.cwd() }) {
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) throw new Error("Expected repository owner/name.");
  if (!/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(tag)) throw new Error("Expected a version tag such as v0.1.5.");
  root = path.resolve(root);
  const source = insideRoot(root, path.resolve(root, sourcePath));
  const base = `https://github.com/${repository}/blob/${encodeSegment(tag)}/`;
  function destination(original) {
    if (!original || /^(?:[A-Za-z][A-Za-z0-9+.-]*:|\/\/|#|\?)/.test(original)) return original;
    const suffixAt = original.search(/[?#]/);
    const pathname = suffixAt < 0 ? original : original.slice(0, suffixAt);
    const suffix = suffixAt < 0 ? "" : original.slice(suffixAt);
    const decoded = decodeURIComponent(pathname.replace(/\\([\\()[\]<>])/g, "$1"));
    if (decoded.includes("\\")) throw new Error("Use forward slashes in release-note links.");
    const target = path.resolve(root, decoded.startsWith("/") ? `.${decoded}` : path.posix.join(path.posix.dirname(source), decoded));
    const relative = insideRoot(root, target);
    let exists = false;
    try { exists = statSync(target).isFile(); } catch { /* Report a concise missing-target error. */ }
    if (!exists) throw new Error(`Release-note target is not a file: ${relative}`);
    return base + relative.split("/").map(encodeSegment).join("/") + suffix;
  }
  const prose = proseOnly(markdown);
  const replacements = [];
  for (let i = 0; i < prose.length; i++) {
    if (prose[i] !== "[" || escaped(prose, i)) continue;
    let depth = 1;
    let close = i + 1;
    for (; close < prose.length; close++) {
      if (escaped(prose, close)) continue;
      if (prose[close] === "[") depth++;
      if (prose[close] === "]" && --depth === 0) break;
    }
    if (prose[close + 1] !== "(") continue;
    const span = inlineDestination(prose, close + 1);
    if (!span) throw new Error("Unsupported inline link in release notes; use a standard Markdown destination.");
    replacements.push({ ...span, value: destination(markdown.slice(span.start, span.end)) });
    i = span.closing;
  }
  // Reference-style destinations (including images) share the same path rules.
  for (const match of prose.matchAll(/^ {0,3}(\[[^\]\r\n]+\]:[ \t]*)(?:<([^>\r\n]*)>|(\S+))/gm)) {
    const value = match[2] ?? match[3];
    const start = match.index + match[1].length + (match[2] === undefined ? 0 : 1);
    replacements.push({ start, end: start + value.length, value: destination(value) });
  }
  let result = markdown;
  for (const change of replacements.sort((a, b) => b.start - a.start)) result = result.slice(0, change.start) + change.value + result.slice(change.end);
  return result;
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try {
    const { values } = parseArgs({ options: Object.fromEntries(["input", "output", "repository", "tag"].map(key => [key, { type: "string" }])) });
    for (const key of ["input", "output", "repository", "tag"]) if (!values[key]) throw new Error(`Missing --${key}.`);
    const input = realpathSync(values.input);
    let output = path.resolve(values.output);
    try { output = realpathSync(output); } catch { /* A new generated output is expected. */ }
    if (process.platform === "win32" ? input.toLowerCase() === output.toLowerCase() : input === output) throw new Error("Generated notes must not overwrite the source file.");
    const result = prepareReleaseNotes({ markdown: readFileSync(values.input, "utf8"), sourcePath: values.input, repository: values.repository, tag: values.tag });
    mkdirSync(path.dirname(path.resolve(values.output)), { recursive: true });
    writeFileSync(values.output, result, "utf8");
    console.log("Prepared tag-pinned release notes.");
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
