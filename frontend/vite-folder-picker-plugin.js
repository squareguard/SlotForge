import { spawn } from "node:child_process";
import { readdir, stat } from "node:fs/promises";
import { platform } from "node:os";
import path from "node:path";

/** Matches Rust `discovery_service::contains_save_files` extensions. */
const SAVE_EXTENSIONS = new Set(["sav", "save", "dat", "bak", "profile", "json"]);

const MAX_SCAN_DEPTH = 4;

const PICK_FOLDER_PS = `
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.FolderBrowserDialog
$dialog.Description = 'Select the folder containing your game save files'
$dialog.ShowNewFolderButton = $true
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
  [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
  [Console]::Out.WriteLine($dialog.SelectedPath)
}
`.trim();

/**
 * @param {import('http').IncomingMessage} req
 * @returns {Promise<Record<string, unknown>>}
 */
function readJsonBody(req) {
  return new Promise((resolve, reject) => {
    let data = "";
    req.on("data", (chunk) => {
      data += chunk;
    });
    req.on("end", () => {
      try {
        resolve(data ? JSON.parse(data) : {});
      } catch (err) {
        reject(err);
      }
    });
    req.on("error", reject);
  });
}

/**
 * @param {string} filePath
 * @returns {boolean}
 */
function isSaveFileName(filePath) {
  const ext = path.extname(filePath).slice(1).toLowerCase();
  return SAVE_EXTENSIONS.has(ext);
}

/**
 * @param {string} rootPath
 * @returns {Promise<{ name: string, absolutePath: string, relativePath: string, size: number, modifiedAt: string }[]>}
 */
async function scanSaveDirectoryOnDisk(rootPath) {
  const root = path.resolve(rootPath);
  const rootStat = await stat(root).catch(() => null);
  if (!rootStat?.isDirectory()) {
    return [];
  }

  /** @type {{ name: string, absolutePath: string, relativePath: string, size: number, modifiedAt: string }[]} */
  const found = [];

  /**
   * @param {string} dir
   * @param {number} depth
   */
  async function walk(dir, depth) {
    if (depth > MAX_SCAN_DEPTH) return;

    let entries;
    try {
      entries = await readdir(dir, { withFileTypes: true });
    } catch {
      return;
    }

    for (const entry of entries) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        await walk(full, depth + 1);
        continue;
      }
      if (!entry.isFile() || !isSaveFileName(full)) continue;

      try {
        const info = await stat(full);
        found.push({
          name: entry.name,
          absolutePath: full,
          relativePath: path.relative(root, full).split(path.sep).join("/"),
          size: info.size,
          modifiedAt: info.mtime.toISOString(),
        });
      } catch {
        // skip unreadable files
      }
    }
  }

  await walk(root, 0);
  found.sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }));
  return found;
}

/**
 * Windows-only: folder picker + save-directory scan for dev/preview.
 * @returns {import('vite').Plugin}
 */
export function folderPickerPlugin() {
  const attach = (server) => {
    if (platform() !== "win32") return;

    server.middlewares.use("/api/pick-folder", (req, res) => {
      if (req.method !== "POST" && req.method !== "GET") {
        res.statusCode = 405;
        res.end(JSON.stringify({ ok: false, error: "Method not allowed" }));
        return;
      }

      const child = spawn(
        "powershell.exe",
        ["-NoProfile", "-STA", "-Command", PICK_FOLDER_PS],
        { windowsHide: false }
      );

      let stdout = "";
      let stderr = "";

      child.stdout.on("data", (chunk) => {
        stdout += chunk.toString("utf8");
      });
      child.stderr.on("data", (chunk) => {
        stderr += chunk.toString("utf8");
      });

      child.on("error", (err) => {
        res.statusCode = 500;
        res.setHeader("Content-Type", "application/json");
        res.end(JSON.stringify({ ok: false, error: err.message }));
      });

      child.on("close", (code) => {
        res.setHeader("Content-Type", "application/json");
        const picked = stdout.trim();
        if (picked && code === 0) {
          res.end(JSON.stringify({ ok: true, path: picked }));
          return;
        }
        if (!picked) {
          res.end(JSON.stringify({ ok: false, cancelled: true }));
          return;
        }
        res.statusCode = 500;
        res.end(
          JSON.stringify({
            ok: false,
            error: stderr.trim() || `Folder picker exited with code ${code}`,
          })
        );
      });
    });

    server.middlewares.use("/api/scan-save-directory", async (req, res) => {
      if (req.method !== "POST") {
        res.statusCode = 405;
        res.setHeader("Content-Type", "application/json");
        res.end(JSON.stringify({ ok: false, error: "Method not allowed" }));
        return;
      }

      try {
        const body = await readJsonBody(req);
        const dirPath = typeof body.path === "string" ? body.path.trim() : "";
        if (!dirPath) {
          res.statusCode = 400;
          res.setHeader("Content-Type", "application/json");
          res.end(JSON.stringify({ ok: false, error: "path is required" }));
          return;
        }

        const files = await scanSaveDirectoryOnDisk(dirPath);
        res.setHeader("Content-Type", "application/json");
        res.end(JSON.stringify({ ok: true, path: path.resolve(dirPath), files }));
      } catch (err) {
        res.statusCode = 500;
        res.setHeader("Content-Type", "application/json");
        res.end(
          JSON.stringify({
            ok: false,
            error: err instanceof Error ? err.message : "Scan failed",
          })
        );
      }
    });
  };

  return {
    name: "slotforge-folder-picker",
    configureServer: attach,
    configurePreviewServer: attach,
  };
}
