// Fetches libmpv for the Windows build from the official shinchiro/mpv-winbuild-cmake releases
// and checks it against the SHA-256 pinned in scripts/libmpv.json.
//
//   pnpm run setup            download the pinned build into src-tauri/lib
//   pnpm run setup -- --pin   pin the newest release: download it, check it against the digest
//                             GitHub publishes for the asset, and record name and SHA-256
//   pnpm run setup -- --pin <release-tag>   pin a specific release instead
//
// Only build time downloads happen here; the app never downloads code at runtime.

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const pinFile = join(root, "scripts", "libmpv.json");
const libDir = join(root, "src-tauri", "lib");
const dllName = "libmpv-2.dll";
const markerFile = join(libDir, "libmpv.source.json");

// The developer package for generic x86-64 CPUs (the "-v3" builds need AVX2).
const ASSET_PATTERN = /^mpv-dev-x86_64-\d{8}-git-[0-9a-f]+\.7z$/;

function fail(message) {
  console.error(`setup: ${message}`);
  process.exit(1);
}

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

async function download(url) {
  console.log(`setup: downloading ${url}`);
  const response = await fetch(url, { redirect: "follow" });
  if (!response.ok) fail(`download failed with HTTP ${response.status}`);
  return Buffer.from(await response.arrayBuffer());
}

async function pin(pinned, tag) {
  const api = `https://api.github.com/repos/${pinned.repository}/releases/${tag ? `tags/${tag}` : "latest"}`;
  const response = await fetch(api, { headers: { Accept: "application/vnd.github+json" } });
  if (!response.ok) fail(`could not read the release list (HTTP ${response.status})`);
  const release = await response.json();
  const asset = release.assets.find((a) => ASSET_PATTERN.test(a.name));
  if (!asset) fail(`release ${release.tag_name} has no ${ASSET_PATTERN} asset`);
  if (!asset.digest?.startsWith("sha256:")) {
    fail(`GitHub publishes no SHA-256 for ${asset.name}; pick a newer release`);
  }
  const bytes = await download(asset.browser_download_url);
  const actual = sha256(bytes);
  if (actual !== asset.digest.slice("sha256:".length)) {
    fail(`checksum mismatch for ${asset.name}: GitHub says ${asset.digest}, file is ${actual}`);
  }
  const next = { ...pinned, release: release.tag_name, file: asset.name, sha256: actual };
  writeFileSync(pinFile, JSON.stringify(next, null, 2) + "\n");
  console.log(`setup: pinned ${asset.name} (${actual}). Commit scripts/libmpv.json.`);
  return { pinned: next, bytes };
}

/** 7-Zip from PATH, or from its default install folders when it is installed but not on PATH. */
function sevenZip() {
  const candidates = ["7z"];
  for (const base of [process.env.ProgramFiles, process.env["ProgramFiles(x86)"]]) {
    if (base) candidates.push(join(base, "7-Zip", "7z.exe"));
  }
  for (const candidate of candidates) {
    const probe = spawnSync(candidate, ["i"], { stdio: "ignore" });
    if (!probe.error) return candidate;
  }
  fail("7-Zip was not found; install it (for example `winget install 7zip.7zip`) and try again");
}

function extract(archive) {
  const dir = mkdtempSync(join(tmpdir(), "spaceport-libmpv-"));
  const archivePath = join(dir, "mpv-dev.7z");
  writeFileSync(archivePath, archive);
  const result = spawnSync(sevenZip(), ["e", archivePath, `-o${dir}`, dllName, "-y"], {
    stdio: "inherit",
  });
  if (result.error) fail(`could not run 7-Zip: ${result.error.message}`);
  if (result.status !== 0 || !existsSync(join(dir, dllName))) {
    fail(`could not extract ${dllName} from the archive`);
  }
  mkdirSync(libDir, { recursive: true });
  copyFileSync(join(dir, dllName), join(libDir, dllName));
  rmSync(dir, { recursive: true, force: true });
}

async function main() {
  const args = process.argv.slice(2).filter((a) => a !== "--");
  let pinned = JSON.parse(readFileSync(pinFile, "utf8"));
  let bytes = null;

  if (args[0] === "--pin") {
    ({ pinned, bytes } = await pin(pinned, args[1]));
  } else if (!pinned.sha256 || !pinned.file || !pinned.release) {
    fail("no libmpv build is pinned yet; run `pnpm run setup -- --pin` once and commit the result");
  }

  if (existsSync(markerFile) && existsSync(join(libDir, dllName))) {
    const marker = JSON.parse(readFileSync(markerFile, "utf8"));
    if (marker.sha256 === pinned.sha256) {
      console.log(`setup: ${dllName} from ${pinned.file} is already in place`);
      return;
    }
  }

  if (!bytes) {
    const url = `https://github.com/${pinned.repository}/releases/download/${pinned.release}/${pinned.file}`;
    bytes = await download(url);
    const actual = sha256(bytes);
    if (actual !== pinned.sha256) {
      fail(`checksum mismatch: pinned ${pinned.sha256}, downloaded ${actual}; not using it`);
    }
  }
  extract(bytes);
  writeFileSync(markerFile, JSON.stringify({ file: pinned.file, sha256: pinned.sha256 }, null, 2) + "\n");
  console.log(`setup: installed ${dllName} from ${pinned.file}`);
}

main().catch((e) => fail(e.message));
