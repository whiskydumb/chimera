/** fetch JSON with a timeout and basic error handling. */
export async function fetchJson(url, { timeoutMs = 5000, ...init } = {}) {
  const controller = new AbortController();
  const id = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const res = await fetch(url, { ...init, signal: controller.signal });
    if (!res.ok) throw new Error(`HTTP ${res.status} for ${url}`);
    return await res.json();
  } finally {
    clearTimeout(id);
  }
}
