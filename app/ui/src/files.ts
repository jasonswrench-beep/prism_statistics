// Choosing where projects live. Inside the desktop window this uses the system's open and save dialogs; in a browser
// (the test server) it returns null and the caller asks for a path in a plain dialog instead.

import { inTauri } from './bridge';

export const PROJECT_EXT = 'pstat';

export async function chooseProjectToOpen(): Promise<string | null | undefined> {
  if (!inTauri()) return undefined;
  const { open } = await import('@tauri-apps/plugin-dialog');
  const picked = await open({ multiple: false, directory: false, filters: [{ name: 'PrismStatistics project', extensions: [PROJECT_EXT] }] });
  return typeof picked === 'string' ? picked : null;
}

export async function chooseProjectToSave(suggested: string): Promise<string | null | undefined> {
  if (!inTauri()) return undefined;
  const { save } = await import('@tauri-apps/plugin-dialog');
  const picked = await save({ defaultPath: `${suggested}.${PROJECT_EXT}`, filters: [{ name: 'PrismStatistics project', extensions: [PROJECT_EXT] }] });
  return picked ?? null;
}

/** Where to save an export. undefined in a browser, where the file is downloaded instead. */
export async function chooseExportPath(format: 'docx' | 'html' | 'pdf' | 'r', suggested: string): Promise<string | null | undefined> {
  if (!inTauri()) return undefined;
  const { save } = await import('@tauri-apps/plugin-dialog');
  const name = format === 'docx' ? 'Word document' : format === 'pdf' ? 'PDF (tagged, accessible)' : format === 'r' ? 'R script' : 'Web page';
  const ext = format === 'r' ? 'R' : format;
  const picked = await save({ defaultPath: `${suggested}.${ext}`, filters: [{ name, extensions: [ext] }] });
  return picked ?? null;
}

/** Where to save the dataset itself (File › Export › Data). undefined in a browser, where the file is downloaded. */
export async function chooseDataPath(ext: 'csv' | 'sav', suggested: string): Promise<string | null | undefined> {
  if (!inTauri()) return undefined;
  const { save } = await import('@tauri-apps/plugin-dialog');
  const picked = await save({ defaultPath: `${suggested.replace(/[^A-Za-z0-9 \-_]+/g, ' ').replace(/\s+/g, ' ').trim() || 'data'}.${ext}`, filters: [{ name: ext === 'csv' ? 'CSV data' : 'SAV data file', extensions: [ext] }] });
  return picked ?? null;
}

/** Where to save one figure or table. undefined in a browser, where the file is downloaded instead. */
export async function chooseItemPath(ext: 'png' | 'svg' | 'csv', suggested: string): Promise<string | null | undefined> {
  if (!inTauri()) return undefined;
  const { save } = await import('@tauri-apps/plugin-dialog');
  const name = ext === 'png' ? 'PNG image' : ext === 'svg' ? 'SVG image' : 'CSV table';
  const picked = await save({ defaultPath: `${suggested.replace(/[^A-Za-z0-9 \-]+/g, ' ').replace(/\s+/g, ' ').trim()}.${ext}`, filters: [{ name, extensions: [ext] }] });
  return picked ?? null;
}

/** Hands bytes to the browser as a download (test server only). */
export function download(fileName: string, base64: string, type: string) {
  const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
  const url = URL.createObjectURL(new Blob([bytes], { type }));
  const a = document.createElement('a');
  a.href = url;
  a.download = fileName;
  document.body.appendChild(a);
  a.click();
  a.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** Reads a chosen file as base64 for the engine. */
export async function fileToBase64(file: File): Promise<string> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  let binary = '';
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  return btoa(binary);
}
