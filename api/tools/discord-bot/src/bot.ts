import { Client, Events, GatewayIntentBits } from "discord.js";
import { replyWithPool } from "./pool.js";

const token = process.env.DISCORD_TOKEN;
if (!token) throw new Error("Set DISCORD_TOKEN before starting the bot.");

const client = new Client({ intents: [GatewayIntentBits.Guilds] });
client.once(Events.ClientReady, (readyClient) => {
  console.log(`Connected as ${readyClient.user.tag}`);
});
client.on(Events.InteractionCreate, async (interaction) => {
  if (!interaction.isChatInputCommand() || interaction.commandName !== "pool") return;
  await replyWithPool(interaction);
});

await client.login(token);
