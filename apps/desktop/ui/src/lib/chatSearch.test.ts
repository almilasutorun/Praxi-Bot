import assert from "node:assert/strict";
import { test } from "node:test";
import { filterChats } from "./chatSearch.ts";

const chats = [{ title: "Trip to Rome" }, { title: "Tax return" }, { title: "" }];

test("filters by displayed title, ignoring case, and keeps all for a blank query", () => {
  assert.deepEqual(filterChats(chats, "ROME", "New chat"), [chats[0]]);
  assert.equal(filterChats(chats, "  ", "New chat").length, 3);
  assert.deepEqual(filterChats(chats, "zzz", "New chat"), []);
});

test("an untitled chat is found by its placeholder label", () => {
  assert.deepEqual(filterChats(chats, "new", "New chat"), [chats[2]]);
});

test("titles with Turkish letters are found with plain ASCII or either i", () => {
  const tr = [{ title: "İstanbul gezisi" }, { title: "Çay saati" }, { title: "ışık" }];
  assert.deepEqual(filterChats(tr, "istanbul", "New chat"), [tr[0]]);
  assert.deepEqual(filterChats(tr, "İSTANBUL", "New chat"), [tr[0]]);
  assert.deepEqual(filterChats(tr, "cay", "New chat"), [tr[1]]);
  assert.deepEqual(filterChats(tr, "isik", "New chat"), [tr[2]]);
});
