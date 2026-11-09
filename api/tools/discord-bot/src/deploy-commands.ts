import { REST, Routes, SlashCommandBuilder } from "discord.js";

const token = process.env.DISCORD_TOKEN;
const applicationId = process.env.DISCORD_APPLICATION_ID;
const guildId = process.env.DISCORD_GUILD_ID;

if (!token || !applicationId) {
  throw new Error("Set DISCORD_TOKEN and DISCORD_APPLICATION_ID before deploying commands.");
}

const commands = [
  new SlashCommandBuilder()
    .setName("pool")
    .setDescription("Show the latest BDL Pool status and statistics")
    .toJSON(),
];
const rest = new REST({ version: "10" }).setToken(token);
const route = guildId
  ? Routes.applicationGuildCommands(applicationId, guildId)
  : Routes.applicationCommands(applicationId);

await rest.put(route, { body: commands });
console.log(guildId ? "Guild slash command registered." : "Global slash command registered.");
