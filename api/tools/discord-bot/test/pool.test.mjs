import test from "node:test";
import assert from "node:assert/strict";
import { replyWithPool } from "../dist/pool.js";

test("an expired Discord interaction does not escape the command handler", async () => {
  const errors = [];
  let edits = 0;
  const interaction = {
    async deferReply() { throw new Error("Unknown interaction"); },
    async editReply() { edits++; },
  };
  await assert.doesNotReject(() => replyWithPool(interaction, error => errors.push(error)));
  assert.equal(edits, 0);
  assert.equal(errors.length, 1);
});

test("a failed Discord error reply does not reject the event handler", async (t) => {
  t.mock.method(globalThis, "fetch", async () => { throw new Error("API unavailable"); });
  const errors = [];
  const interaction = {
    async deferReply() {},
    async editReply() { throw new Error("Discord unavailable"); },
  };
  await assert.doesNotReject(() => replyWithPool(interaction, error => errors.push(error)));
  assert.equal(errors.length, 1);
});

test("a valid pool response still produces a status embed", async (t) => {
  t.mock.method(globalThis, "fetch", async () => new Response(JSON.stringify({
    apiVersion:"1",updatedAt:null,ageSeconds:1,stale:false,
    pool:{hashrateHps:1000,workers:1,bestShare:5,blocksFound:0,readyAtLastCheck:true},
    node:{initialBlockDownload:false,stale:false},checks:{stratum:"job_received"},
  }), {status:200}));
  const replies = [];
  await replyWithPool({async deferReply() {},async editReply(value) { replies.push(value); }});
  assert.equal(replies.length, 1);
  assert.equal(replies[0].embeds[0].toJSON().description, "Prêt au dernier contrôle");
});
