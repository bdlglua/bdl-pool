const apiBaseUrl = (process.env.BDL_POOL_API_URL ?? "https://api.bdl-pool.com:36018/api/v1").replace(/\/$/, "");

type Health = {
  apiVersion: "1";
  status: "ok";
};

function isHealth(value: unknown): value is Health {
  if (typeof value !== "object" || value === null) return false;
  const health = value as Record<string, unknown>;
  return health.apiVersion === "1" && health.status === "ok";
}

async function main(): Promise<void> {
  const response = await fetch(`${apiBaseUrl}/health`, { signal: AbortSignal.timeout(8000) });
  if (!response.ok) throw new Error(`API returned HTTP ${response.status}`);

  const payload: unknown = await response.json();
  if (!isHealth(payload)) throw new Error("API returned an invalid health response");

  console.log(`BDL Pool API ${payload.apiVersion}: ${payload.status}`);
}

main().catch((error: unknown) => {
  console.error(error instanceof Error ? error.message : "API request failed");
  process.exitCode = 1;
});
