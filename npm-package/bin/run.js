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
} else if (subcommand === 'uninstall') {
  uninstall();
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
  var VERSION = '0.8.19';
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
        checkDuplicateBinary(destPath);
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

  // Check for duplicate binaries at other locations
  checkDuplicateBinary(destPath);
}

// Find the best install directory that is already in the user's PATH.
// Returns the first writable candidate in PATH, or falls back to ~/.local/bin.
function findBestInstallDir() {
  var home = process.env.HOME || process.env.USERPROFILE || '';
  var isWindows = process.platform === 'win32';
  var pathSep = isWindows ? ';' : ':';
  var pathDirs = (process.env.PATH || '').split(pathSep).filter(Boolean);

  // Primary: ~/.squad/bin — our own directory, does not need to be on PATH
  var squadBin = path.join(home, '.squad', 'bin');
  try {
    fs.mkdirSync(squadBin, { recursive: true });
    fs.accessSync(squadBin, fs.constants.W_OK);
    return squadBin;
  } catch (_) {
    // Fall through to other candidates
  }

  // Secondary candidates — must be on PATH and writable
  var candidates = isWindows
    ? [
        path.join(home, '.local', 'bin'),
        path.join(home, 'AppData', 'Local', 'Microsoft', 'WindowsApps'),
      ]
    : [
        path.join(home, '.local', 'bin'),
        '/usr/local/bin',
      ];

  for (var i = 0; i < candidates.length; i++) {
    var dir = candidates[i];
    var inPath = pathDirs.some(function(p) {
      return path.resolve(p) === path.resolve(dir);
    });
    if (!inPath) continue;

    try {
      fs.mkdirSync(dir, { recursive: true });
      fs.accessSync(dir, fs.constants.W_OK);
      return dir;
    } catch (_) {
      continue;
    }
  }

  // Fallback: ~/.squad/bin (already created above if writable)
  fs.mkdirSync(squadBin, { recursive: true });
  return squadBin;
}

// Verify the installed binary is callable. If not, auto-add to shell profile.
function verifyInPath(destPath, installDir) {
  var isWindows = process.platform === 'win32';
  var checkCmd = isWindows ? 'where' : 'which';
  var checkResult = spawnSync(checkCmd, ['squad-station'], { encoding: 'utf8' });

  if (checkResult.status === 0 && checkResult.stdout && checkResult.stdout.trim()) {
    var foundPath = checkResult.stdout.trim();
    // Ignore npx cache / node_modules wrappers — not real user PATH entries
    if (!foundPath.includes('.npm/_npx') && !foundPath.includes('node_modules/.bin') && !foundPath.includes('node_modules\\.bin')) {
      // Binary is found in real PATH — all good
      return;
    }
  }

  if (isWindows) {
    // Windows: print manual instructions (modifying registry is too invasive)
    console.log('');
    console.log('  \x1b[33m⚠  squad-station is not in your PATH\x1b[0m');
    console.log('  \x1b[2m# Windows (PowerShell) — run as Administrator:\x1b[0m');
    console.log('  \x1b[36m[Environment]::SetEnvironmentVariable("Path",\x1b[0m');
    console.log('  \x1b[36m  [Environment]::GetEnvironmentVariable("Path", "User") + ";' + installDir + '", "User")\x1b[0m');
    console.log('  Then restart your terminal.');
    console.log('');
    return;
  }

  // macOS/Linux: auto-add to shell profile
  var home = process.env.HOME || '';
  var exportLine = 'export PATH="$HOME/.squad/bin:$PATH"';
  var profileCandidates = process.platform === 'darwin'
    ? ['.zshrc', '.bash_profile', '.bashrc']
    : ['.bashrc', '.zshrc', '.profile'];

  // Find the first existing profile, or default to the platform's primary
  var profileName = profileCandidates[0];
  for (var i = 0; i < profileCandidates.length; i++) {
    if (fs.existsSync(path.join(home, profileCandidates[i]))) {
      profileName = profileCandidates[i];
      break;
    }
  }
  var profilePath = path.join(home, profileName);

  // Check if already added (idempotent)
  var alreadyAdded = false;
  try {
    var content = fs.readFileSync(profilePath, 'utf8');
    if (content.includes('.squad/bin')) {
      alreadyAdded = true;
    }
  } catch (_) {
    // File doesn't exist yet — we'll create it
  }

  if (!alreadyAdded) {
    try {
      var marker = '\n# Squad Station\n' + exportLine + '\n';
      fs.appendFileSync(profilePath, marker);
      console.log('  \x1b[32m✓\x1b[0m Added ~/.squad/bin to PATH in ~/' + profileName);
    } catch (e) {
      console.log('  \x1b[33m⚠\x1b[0m Could not update ~/' + profileName + ': ' + e.message);
      console.log('    Add manually: \x1b[36m' + exportLine + '\x1b[0m');
      console.log('');
      return;
    }
  }

  // Source the profile so it takes effect in the current npx process isn't needed,
  // but we need the user's NEXT terminal to work. Print a note.
  console.log('  \x1b[33m→\x1b[0m Run \x1b[36msource ~/' + profileName + '\x1b[0m or open a new terminal to activate.');
  console.log('');
}

// Check if another squad-station binary exists at a different path than where we installed.
function checkDuplicateBinary(installedPath) {
  var isWindows = process.platform === 'win32';
  var checkCmd = isWindows ? 'where' : 'which';
  var result = spawnSync(checkCmd, ['squad-station'], { encoding: 'utf8' });

  if (result.status !== 0 || !result.stdout || !result.stdout.trim()) {
    return; // not in PATH at all — verifyInPath already warned
  }

  var whichPath = result.stdout.trim();

  // Ignore npx cache / node_modules wrappers — these are not real installs
  if (whichPath.includes('.npm/_npx') || whichPath.includes('node_modules/.bin') || whichPath.includes('node_modules\\.bin')) {
    return;
  }

  var resolvedWhich = fs.realpathSync(whichPath);
  var resolvedInstalled = fs.realpathSync(installedPath);

  if (resolvedWhich !== resolvedInstalled) {
    console.log('');
    console.log('  \x1b[33m⚠  Another squad-station found at: ' + whichPath + '\x1b[0m');
    console.log('     This version will be used instead of the one just installed.');
    console.log('     Remove it to avoid conflicts: \x1b[36mrm ' + whichPath + '\x1b[0m');
    console.log('');
  }
}

// ── Uninstall ───────────────────────────────────────────────────────
// Find and remove all squad-station binaries from known locations.

function uninstall() {
  var home = process.env.HOME || process.env.USERPROFILE || '';
  var isWindows = process.platform === 'win32';
  var binaryName = isWindows ? 'squad-station.exe' : 'squad-station';

  // Search directories: current install locations + legacy locations
  var searchDirs = isWindows
    ? [
        path.join(home, '.squad', 'bin'),
        path.join(home, '.local', 'bin'),
        path.join(home, 'AppData', 'Local', 'Microsoft', 'WindowsApps'),
      ]
    : [
        path.join(home, '.squad', 'bin'),
        path.join(home, '.local', 'bin'),
        '/usr/local/bin',
        path.join(home, '.cargo', 'bin'),
        '/opt/homebrew/bin',
      ];

  console.log('\n\x1b[32m══════════════════════════════════\x1b[0m');
  console.log('  \x1b[1mSquad Station Uninstall\x1b[0m');
  console.log('\x1b[32m══════════════════════════════════\x1b[0m\n');

  // Find all existing binaries
  var found = [];
  for (var i = 0; i < searchDirs.length; i++) {
    var binPath = path.join(searchDirs[i], binaryName);
    if (fs.existsSync(binPath)) {
      var version = '(unknown version)';
      try {
        var result = spawnSync(binPath, ['--version'], { encoding: 'utf8', timeout: 5000 });
        if (result.stdout && result.stdout.trim()) {
          version = result.stdout.trim();
        }
      } catch (_) {}
      found.push({ path: binPath, version: version });
    }
  }

  if (found.length === 0) {
    console.log('  No squad-station binaries found.\n');
    return;
  }

  console.log('  Found ' + found.length + ' binary(ies):\n');
  for (var j = 0; j < found.length; j++) {
    console.log('    \x1b[36m' + found[j].path + '\x1b[0m  ' + found[j].version);
  }
  console.log('');

  // Ask for confirmation (read single line from stdin)
  process.stdout.write('  Remove these binaries? [y/N] ');
  var buf = Buffer.alloc(128);
  var bytesRead = 0;
  try {
    bytesRead = fs.readSync(0, buf, 0, buf.length);
  } catch (_) {
    console.log('\n  Aborted.\n');
    return;
  }
  var answer = buf.toString('utf8', 0, bytesRead).trim().toLowerCase();

  if (answer !== 'y' && answer !== 'yes') {
    console.log('  Aborted.\n');
    return;
  }

  // Delete each binary
  var removed = 0;
  for (var k = 0; k < found.length; k++) {
    try {
      fs.unlinkSync(found[k].path);
      console.log('  \x1b[32m✓\x1b[0m Removed ' + found[k].path);
      removed++;
    } catch (e) {
      console.log('  \x1b[31m✗\x1b[0m Failed to remove ' + found[k].path + ': ' + e.message);
    }
  }

  console.log('');
  if (removed > 0) {
    console.log('  Uninstalled. You can also remove ~/.squad/bin from your PATH if no longer needed.\n');
  }
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

  // Copy rules/ (git workflow rules) — always overwrite with latest
  var rulesSrc = path.join(srcSquad, 'rules');
  if (fs.existsSync(rulesSrc)) {
    var rulesDest = path.join(destSquad, 'rules');
    fs.mkdirSync(rulesDest, { recursive: true });

    var rulesFiles = fs.readdirSync(rulesSrc).filter(function(f) { return f.endsWith('.md'); });
    rulesFiles.forEach(function(file) {
      var dest = path.join(rulesDest, file);
      fs.copyFileSync(path.join(rulesSrc, file), dest);
      console.log('  \x1b[32m✓\x1b[0m .squad/rules/' + file);
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

  // Copy examples/ — always overwrite with latest reference templates
  var exSrc = path.join(srcSquad, 'examples');
  var exDest = path.join(destSquad, 'examples');
  fs.mkdirSync(exDest, { recursive: true });

  var exFiles = fs.readdirSync(exSrc).filter(function(f) { return f.endsWith('.yml'); });
  exFiles.forEach(function(file) {
    var dest = path.join(exDest, file);
    fs.copyFileSync(path.join(exSrc, file), dest);
    console.log('  \x1b[32m✓\x1b[0m .squad/examples/' + file);
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
