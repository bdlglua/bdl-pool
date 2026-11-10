import { EmbedBuilder, type ChatInputCommandInteraction } from "discord.js";

const apiBaseUrl = (process.env.BDL_POOL_API_URL ?? "https://api.bdl-pool.com:36018/api/v1").replace(/\/$/, "");

type PoolSnapshot = {
  apiVersion: "1";
  updatedAt: string | null;
  ageSeconds: number | null;
  stale: boolean;
  pool: {
    hashrateHps: number | null;
    workers: number | null;
    bestShare: number | null;
    blocksFound: number | null;
    readyAtLastCheck: boolean;
  };
  node: {
    initialBlockDownload: boolean | null;
    stale: boolean;
  };
  checks: { stratum: string };
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function nullableNumber(value: unknown): value is number | null {
  return value === null || (typeof value === "number" && Number.isFinite(value) && value >= 0);
}

function isPoolSnapshot(value: unknown): value is PoolSnapshot {
  if (!isRecord(value) || !isRecord(value.pool) || !isRecord(value.node) || !isRecord(value.checks)) return false;
  const pool = value.pool;
  const node = value.node;
  return value.apiVersion === "1"
    && (typeof value.updatedAt === "string" || value.updatedAt === null)
    && nullableNumber(value.ageSeconds)
    && typeof value.stale === "boolean"
    && nullableNumber(pool.hashrateHps)
    && nullableNumber(pool.workers)
    && nullableNumber(pool.bestShare)
    && nullableNumber(pool.blocksFound)
    && typeof pool.readyAtLastCheck === "boolean"
    && (typeof node.initialBlockDownload === "boolean" || node.initialBlockDownload === null)
    && typeof node.stale === "boolean"
    && typeof value.checks.stratum === "string";
}

function formatHashrate(value: number | null): string {
  if (value === null) return "Inconnu";
  const units = ["H/s", "kH/s", "MH/s", "GH/s", "TH/s", "PH/s", "EH/s"];
  let rate = value;
  let unit = 0;
  while (rate >= 1000 && unit < units.length - 1) {
    rate /= 1000;
    unit += 1;
  }
  return `${rate.toLocaleString("fr-FR", { maximumFractionDigits: 2 })} ${units[unit]}`;
}

async function getPool(): Promise<PoolSnapshot> {
  const response = await fetch(`${apiBaseUrl}/pool`, { signal: AbortSignal.timeout(8000) });
  if (!response.ok) throw new Error(`L’API répond HTTP ${response.status}`);
  const value: unknown = await response.json();
  if (!isPoolSnapshot(value)) throw new Error("Réponse /pool invalide");
  return value;
}

function makeEmbed(snapshot: PoolSnapshot): EmbedBuilder {
  const stale = snapshot.stale || snapshot.node.stale;
  const state = stale
    ? "Données périmées"
    : snapshot.node.initialBlockDownload === true
      ? "Nœud en synchronisation"
      : snapshot.pool.readyAtLastCheck
        ? "Prêt au dernier contrôle"
        : "Disponibilité non confirmée";
  const color = stale ? 0xd97706 : snapshot.pool.readyAtLastCheck ? 0x16803c : 0x64748b;
  const embed = new EmbedBuilder()
    .setColor(color)
    .setTitle("BDL Pool")
    .setDescription(state)
    .addFields(
      { name: "Hashrate", value: formatHashrate(snapshot.pool.hashrateHps), inline: true },
      { name: "Workers", value: snapshot.pool.workers?.toLocaleString("fr-FR") ?? "Inconnu", inline: true },
      { name: "Blocs trouvés", value: snapshot.pool.blocksFound?.toLocaleString("fr-FR") ?? "Inconnu", inline: true },
      { name: "Meilleure share", value: snapshot.pool.bestShare?.toLocaleString("fr-FR") ?? "Inconnue", inline: true },
      { name: "Contrôle Stratum V1", value: snapshot.checks.stratum, inline: true },
      { name: "Âge du relevé", value: snapshot.ageSeconds === null ? "Inconnu" : `${snapshot.ageSeconds} s`, inline: true },
    )
    .setFooter({ text: "État selon le dernier contrôle; cela ne confirme pas la disponibilité actuelle de Stratum V2." });
  if (snapshot.updatedAt && Number.isFinite(Date.parse(snapshot.updatedAt))) {
    embed.setTimestamp(new Date(snapshot.updatedAt));
  }
  return embed;
}

async function replyWithPool(
  interaction: ChatInputCommandInteraction,
  reportError: (error: unknown) => void = error => console.error(
    "Pool command failed:", error instanceof Error ? error.message : "Discord request failed",
  ),
): Promise<void> {
  try {
    await interaction.deferReply({ ephemeral: true });
  } catch (error) {
    reportError(error);
    return;
  }
  try {
    const snapshot = await getPool();
    await interaction.editReply({ embeds: [makeEmbed(snapshot)] });
  } catch (error) {
    const message = error instanceof Error ? error.message : "Erreur de lecture de l’API";
    try {
      await interaction.editReply(`Impossible de lire l’état du pool : ${message}`);
    } catch (replyError) {
      reportError(replyError);
    }
  }
}


export { replyWithPool, isPoolSnapshot, makeEmbed };
