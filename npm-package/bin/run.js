#!/usr/bin/env node
// bin/run.js — entry point for npx squad-station
// Handles "install" in JS (download binary + scaffold project files).
// All other subcommands proxy to the native Rust binary.

const { spawnSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const subcommand = process.argv[2];

if (subcommand === 'install') {
  install();
} else {
  proxyToBinary();
}

// ── Install ─────────────────────────────────────────────────────────
// 1. Download squad-station binary to system PATH
// 2. Copy .squad/ project files to CWD

function install() {
  var args = process.argv.slice(3);
  var force = args.includes('--force') || args.includes('-f');

  console.log('\n\x1b[32m══════════════════════════════════\x1b[0m');
  console.log('  \x1b[1mSquad Station Install\x1b[0m');
  console.log('\x1b[32m══════════════════════════════════\x1b[0m\n');

  // Step 1: Install binary
  installBinary();

  // Step 2: Scaffold project files
  scaffoldProject(force);

  // Done
  console.log('\n\x1b[1mNext steps:\x1b[0m');
  console.log('  1. Copy an example config:');
  console.log('     \x1b[36mcp .squad/examples/orchestrator-claude.yml squad.yml\x1b[0m');
  console.log('  2. Edit \x1b[36msquad.yml\x1b[0m — set project name, providers, models');
  console.log('  3. Run  \x1b[36msquad-station init\x1b[0m — launch tmux sessions\n');
}

function installBinary() {
  // Binary version — may differ from npm package version
  var VERSION = '0.7.23';
  var REPO = 'thientranhung/squad-station';

  var isWindows = process.platform === 'win32';
  var platformMap = { darwin: 'darwin', linux: 'linux', win32: 'windows' };
  var archMap = { x64: 'x86_64', arm64: 'arm64' };

  var p = platformMap[process.platform];
  var a = archMap[process.arch];

  if (!p || !a) {
    console.error('Unsupported platform: ' + process.platform + ' ' + process.arch);
    console.error('Manual install: https://github.com/' + REPO + '/releases');
    process.exit(1);
  }

  var binaryName = isWindows ? 'squad-station.exe' : 'squad-station';
  var assetName = 'squad-station-' + p + '-' + a + (isWindows ? '.exe' : '');
  var url = 'https://github.com/' + REPO + '/releases/download/v' + VERSION + '/' + assetName;

  // Determine best install directory — pick one already in PATH
  var installDir = findBestInstallDir();
  var destPath = path.join(installDir, binaryName);

  // Check if binary already exists and is the right version
  if (fs.existsSync(destPath)) {
    try {
      var result = spawnSync(destPath, ['--version'], { encoding: 'utf8' });
      if (result.stdout && result.stdout.includes(VERSION)) {
        console.log('  \x1b[32m✓\x1b[0m squad-station v' + VERSION + ' already installed at ' + destPath);
        return;
      }
    } catch (_) {
      // Can't check version, re-download
    }
    // Version mismatch — remove old binary (may be symlink from cargo install)
    try { fs.unlinkSync(destPath); } catch (_) {}
  }

  console.log('  Downloading ' + assetName + ' v' + VERSION + '...');

  if (isWindows) {
    // Use PowerShell on Windows
    var psCmd = 'Invoke-WebRequest -Uri "' + url + '" -OutFile "' + destPath + '" -UseBasicParsing';
    var dlResult = spawnSync('powershell', ['-Command', psCmd], { stdio: ['ignore', 'pipe', 'pipe'] });
  } else {
    // Use curl on macOS/Linux
    var dlResult = spawnSync('curl', [
      '-fsSL', '--proto', '=https', '--tlsv1.2',
      '-o', destPath,
      url
    ], { stdio: ['ignore', 'pipe', 'pipe'] });
  }

  if (dlResult.status !== 0) {
    var stderr = dlResult.stderr ? dlResult.stderr.toString() : '';
    console.error('  Download failed: ' + stderr);
    console.error('  Manual install: https://github.com/' + REPO + '/releases');
    process.exit(1);
  }

  if (!isWindows) {
    fs.chmodSync(destPath, 0o755);
    // macOS Gatekeeper: strip quarantine flag so unsigned binary is not killed
    if (process.platform === 'darwin') {
      spawnSync('xattr', ['-d', 'com.apple.quarantine', destPath], { stdio: 'ignore' });
      spawnSync('xattr', ['-d', 'com.apple.provenance', destPath], { stdio: 'ignore' });
    }
  }
  console.log('  \x1b[32m✓\x1b[0m Installed squad-station to ' + destPath);

  // Verify the binary is actually callable via PATH
  verifyInPath(destPath, installDir);
}

// Find the best install directory that is already in the user's PATH.
// Returns the first writable candidate in PATH, or falls back to ~/.local/bin.
function findBestInstallDir() {
  var home = process.env.HOME || process.env.USERPROFILE || '';
  var isWindows = process.platform === 'win32';
  var pathSep = isWindows ? ';' : ':';
  var pathDirs = (process.env.PATH || '').split(pathSep).filter(Boolean);

  // Candidate directories in preference order
  var candidates = isWindows
    ? [
        path.join(home, '.local', 'bin'),
        path.join(home, 'AppData', 'Local', 'Microsoft', 'WindowsApps'),
      ]
    : [
        '/usr/local/bin',
        path.join(home, '.local', 'bin'),
        path.join(home, '.cargo', 'bin'),
        '/opt/homebrew/bin',
      ];

  // Pick the first candidate that is already in PATH and is writable
  for (var i = 0; i < candidates.length; i++) {
    var dir = candidates[i];
    // Check if this directory is in PATH
    var inPath = pathDirs.some(function(p) {
      return path.resolve(p) === path.resolve(dir);
    });
    if (!inPath) continue;

    // Check if writable (create if needed)
    try {
      fs.mkdirSync(dir, { recursive: true });
      fs.accessSync(dir, fs.constants.W_OK);
      return dir;
    } catch (_) {
      continue;
    }
  }

  // Fallback: ~/.local/bin (may not be in PATH — we'll warn later)
  var fallback = path.join(home, '.local', 'bin');
  fs.mkdirSync(fallback, { recursive: true });
  return fallback;
}

// Verify the installed binary is callable. If not, print PATH instructions.
function verifyInPath(destPath, installDir) {
  var isWindows = process.platform === 'win32';
  var checkCmd = isWindows ? 'where' : 'which';
  var checkResult = spawnSync(checkCmd, ['squad-station'], { encoding: 'utf8' });

  if (checkResult.status === 0 && checkResult.stdout && checkResult.stdout.trim()) {
    // Binary is found in PATH — all good
    return;
  }

  // Not in PATH — print platform-specific instructions
  console.log('');
  console.log('  \x1b[33m⚠  squad-station is not in your PATH\x1b[0m');
  console.log('  The binary was installed to: \x1b[36m' + installDir + '\x1b[0m');
  console.log('');
  console.log('  Add it to your PATH:');
  console.log('');

  if (process.platform === 'darwin') {
    console.log('  \x1b[2m# macOS (zsh) — add to ~/.zshrc:\x1b[0m');
    console.log('  \x1b[36mexport PATH="' + installDir + ':$PATH"\x1b[0m');
    console.log('');
    console.log('  Then reload: \x1b[36msource ~/.zshrc\x1b[0m');
  } else if (isWindows) {
    console.log('  \x1b[2m# Windows (PowerShell) — run as Administrator:\x1b[0m');
    console.log('  \x1b[36m[Environment]::SetEnvironmentVariable("Path",\x1b[0m');
    console.log('  \x1b[36m  [Environment]::GetEnvironmentVariable("Path", "User") + ";' + installDir + '", "User")\x1b[0m');
    console.log('');
    console.log('  Then restart your terminal.');
  } else {
    // Linux
    console.log('  \x1b[2m# Linux (bash) — add to ~/.bashrc:\x1b[0m');
    console.log('  \x1b[36mexport PATH="' + installDir + ':$PATH"\x1b[0m');
    console.log('');
    console.log('  Then reload: \x1b[36msource ~/.bashrc\x1b[0m');
  }
  console.log('');
}

function scaffoldProject(force) {
  // Source: bundled .squad/ directory inside npm package
  var pkgRoot = path.join(__dirname, '..');
  var srcSquad = path.join(pkgRoot, '.squad');
  var destSquad = path.join(process.cwd(), '.squad');

  console.log('');

  // Copy sdd/ playbooks
  var sddSrc = path.join(srcSquad, 'sdd');
  var sddDest = path.join(destSquad, 'sdd');
  fs.mkdirSync(sddDest, { recursive: true });

  var sddFiles = fs.readdirSync(sddSrc).filter(function(f) { return f.endsWith('.md'); });
  sddFiles.forEach(function(file) {
    var dest = path.join(sddDest, file);
    fs.copyFileSync(path.join(sddSrc, file), dest);
    console.log('  \x1b[32m✓\x1b[0m .squad/sdd/' + file);
  });

  // Copy rules/ (git workflow rules)
  var rulesSrc = path.join(srcSquad, 'rules');
  if (fs.existsSync(rulesSrc)) {
    var rulesDest = path.join(destSquad, 'rules');
    fs.mkdirSync(rulesDest, { recursive: true });

    var rulesFiles = fs.readdirSync(rulesSrc).filter(function(f) { return f.endsWith('.md'); });
    rulesFiles.forEach(function(file) {
      var dest = path.join(rulesDest, file);
      if (fs.existsSync(dest) && !force) {
        console.log('  \x1b[33m–\x1b[0m .squad/rules/' + file + ' \x1b[2m(exists, use --force to overwrite)\x1b[0m');
      } else {
        fs.copyFileSync(path.join(rulesSrc, file), dest);
        console.log('  \x1b[32m✓\x1b[0m .squad/rules/' + file);
      }
    });
  }

  // Copy hooks/ (notification scripts)
  var hooksSrc = path.join(srcSquad, 'hooks');
  if (fs.existsSync(hooksSrc)) {
    var hooksDest = path.join(destSquad, 'hooks');
    fs.mkdirSync(hooksDest, { recursive: true });

    var hooksFiles = fs.readdirSync(hooksSrc).filter(function(f) { return f.endsWith('.sh'); });
    hooksFiles.forEach(function(file) {
      var dest = path.join(hooksDest, file);
      if (fs.existsSync(dest) && !force) {
        console.log('  \x1b[33m–\x1b[0m .squad/hooks/' + file + ' \x1b[2m(exists, use --force to overwrite)\x1b[0m');
      } else {
        fs.copyFileSync(path.join(hooksSrc, file), dest);
        fs.chmodSync(dest, 0o755);
        console.log('  \x1b[32m✓\x1b[0m .squad/hooks/' + file);
      }
    });
  }

  // Copy examples/
  var exSrc = path.join(srcSquad, 'examples');
  var exDest = path.join(destSquad, 'examples');
  fs.mkdirSync(exDest, { recursive: true });

  var exFiles = fs.readdirSync(exSrc).filter(function(f) { return f.endsWith('.yml'); });
  exFiles.forEach(function(file) {
    var dest = path.join(exDest, file);
    if (fs.existsSync(dest) && !force) {
      console.log('  \x1b[33m–\x1b[0m .squad/examples/' + file + ' \x1b[2m(exists, use --force to overwrite)\x1b[0m');
    } else {
      fs.copyFileSync(path.join(exSrc, file), dest);
      console.log('  \x1b[32m✓\x1b[0m .squad/examples/' + file);
    }
  });
}

// ── Proxy ───────────────────────────────────────────────────────────
// Forward all non-install subcommands to the native binary.

function proxyToBinary() {
  var binaryPath = null;

  // Try system-installed binary via PATH
  var which = spawnSync('which', ['squad-station'], { encoding: 'utf8' });
  if (which.status === 0 && which.stdout) {
    binaryPath = which.stdout.trim();
  }

  if (!binaryPath) {
    console.error('squad-station binary not found.');
    console.error('Run: npx squad-station install');
    process.exit(1);
  }

  var result = spawnSync(binaryPath, process.argv.slice(2), { stdio: 'inherit' });

  if (result.error) {
    console.error(result.error.message);
    process.exit(1);
  }

  process.exit(result.status != null ? result.status : 0);
}
