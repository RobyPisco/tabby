/** Controllo di una nuova versione: confronta la release più recente su GitHub con quella installata. */

const LATEST_RELEASE = "https://api.github.com/repos/RobyPisco/tabby/releases/latest";

export type Release = { version: string; url: string };

/** Numeri di una versione ("v1.2.3" → [1, 2, 3]). */
function parts(version: string): number[] {
  return version.replace(/^v/i, "").split(".").map((n) => parseInt(n, 10) || 0);
}

export function isNewer(candidate: string, current: string): boolean {
  const a = parts(candidate);
  const b = parts(current);
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    const diff = (a[i] ?? 0) - (b[i] ?? 0);
    if (diff !== 0) return diff > 0;
  }
  return false;
}

/** La release più recente, oppure `null` se non ce n'è una più nuova di `current`. Lancia se la rete non risponde. */
export async function findUpdate(current: string): Promise<Release | null> {
  const response = await fetch(LATEST_RELEASE, { headers: { Accept: "application/vnd.github+json" } });
  if (!response.ok) throw new Error(`GitHub ha risposto ${response.status}`);
  const data = (await response.json()) as { tag_name?: string; html_url?: string };
  if (!data.tag_name || !data.html_url) throw new Error("risposta di GitHub non valida");
  const version = data.tag_name.replace(/^v/i, "");
  return isNewer(version, current) ? { version, url: data.html_url } : null;
}
