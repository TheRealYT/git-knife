import { describe, it, expect } from "vitest";
import {
  escapeRegExp,
  checkRegexSafety,
  computeBulk,
  effectiveValue,
  emptyBulkSpec,
  type BulkSpec,
} from "./bulk";
import type { Commit, CommitEdit } from "./types";

/* ── helpers ───────────────────────────────────────────────── */

/** A minimal Commit stub for testing. */
function makeCommit(overrides: Partial<Commit> = {}): Commit {
  return {
    hash: "aaaa1111bbbb2222cccc3333dddd4444eeee5555",
    parents: [],
    authorName: "Alice",
    authorEmail: "alice@example.com",
    authorDate: "2024-01-01T12:00:00+00:00",
    committerName: "Alice",
    committerEmail: "alice@example.com",
    committerDate: "2024-01-01T12:00:00+00:00",
    subject: "Initial commit",
    body: "",
    isMerge: false,
    signed: false,
    ...overrides,
  };
}

/* ── escapeRegExp ──────────────────────────────────────────── */

describe("escapeRegExp", () => {
  it("escapes special regex characters", () => {
    expect(escapeRegExp("foo.bar*baz")).toBe("foo\\.bar\\*baz");
    expect(escapeRegExp("a+b?c")).toBe("a\\+b\\?c");
    expect(escapeRegExp("test[0]")).toBe("test\\[0\\]");
    expect(escapeRegExp("$100")).toBe("\\$100");
    expect(escapeRegExp("(group)")).toBe("\\(group\\)");
    expect(escapeRegExp("{1,2}")).toBe("\\{1,2\\}");
    expect(escapeRegExp("a|b")).toBe("a\\|b");
    expect(escapeRegExp("^start$")).toBe("\\^start\\$");
  });

  it("leaves plain strings unchanged", () => {
    expect(escapeRegExp("hello world")).toBe("hello world");
    expect(escapeRegExp("")).toBe("");
    expect(escapeRegExp("abc123")).toBe("abc123");
  });
});

/* ── checkRegexSafety ──────────────────────────────────────── */

describe("checkRegexSafety", () => {
  it("rejects nested quantifiers", () => {
    expect(checkRegexSafety("(a+)+")).not.toBeNull();
    expect(checkRegexSafety("(.*)*")).not.toBeNull();
    expect(checkRegexSafety("(\\d+)+")).not.toBeNull();
    expect(checkRegexSafety("([a-z]+)+")).not.toBeNull();
  });

  it("rejects overlapping alternations with wild quantifiers", () => {
    expect(checkRegexSafety("(.*|.*)+")).not.toBeNull();
    expect(checkRegexSafety("(.+|x)+")).not.toBeNull();
  });

  it("accepts safe patterns", () => {
    expect(checkRegexSafety("hello")).toBeNull();
    expect(checkRegexSafety("[a-z]+")).toBeNull();
    expect(checkRegexSafety("\\d{3}-\\d{4}")).toBeNull();
    expect(checkRegexSafety("foo|bar")).toBeNull();
  });
});

/* ── effectiveValue ────────────────────────────────────────── */

describe("effectiveValue", () => {
  const commit = makeCommit({
    hash: "aaa",
    authorName: "Alice",
    authorEmail: "alice@example.com",
    subject: "Fix bug",
    body: "Detailed description",
  });

  it("returns original value when no edit exists", () => {
    const edits: Record<string, CommitEdit> = {};
    expect(effectiveValue(commit, "authorName", edits)).toBe("Alice");
    expect(effectiveValue(commit, "authorEmail", edits)).toBe("alice@example.com");
  });

  it("returns edited value when edit exists", () => {
    const edits: Record<string, CommitEdit> = {
      aaa: { hash: "aaa", authorName: "Bob" },
    };
    expect(effectiveValue(commit, "authorName", edits)).toBe("Bob");
    // Unedited field still returns original
    expect(effectiveValue(commit, "authorEmail", edits)).toBe("alice@example.com");
  });

  it("returns full message for message field", () => {
    const edits: Record<string, CommitEdit> = {};
    // fullMessage returns "subject\n\nbody" when body is non-empty
    expect(effectiveValue(commit, "message", edits)).toBe("Fix bug\n\nDetailed description");
  });

  it("returns edited message when present", () => {
    const edits: Record<string, CommitEdit> = {
      aaa: { hash: "aaa", message: "New message" },
    };
    expect(effectiveValue(commit, "message", edits)).toBe("New message");
  });
});

/* ── computeBulk ───────────────────────────────────────────── */

describe("computeBulk", () => {
  const commits: Commit[] = [
    makeCommit({
      hash: "aaa",
      authorName: "Alice",
      authorEmail: "alice@example.com",
      subject: "Fix bug in parser",
      body: "",
    }),
    makeCommit({
      hash: "bbb",
      authorName: "Bob",
      authorEmail: "bob@example.com",
      subject: "Add feature",
      body: "",
    }),
    makeCommit({
      hash: "ccc",
      authorName: "Alice",
      authorEmail: "alice@old.com",
      subject: "Update docs",
      body: "",
    }),
  ];

  const emptyEdits: Record<string, CommitEdit> = {};

  it("returns empty result when find is empty", () => {
    const spec: BulkSpec = { ...emptyBulkSpec, find: "" };
    const result = computeBulk(commits, emptyEdits, spec);
    expect(result.matchedCommits).toBe(0);
    expect(result.replacements).toBe(0);
    expect(result.error).toBeNull();
  });

  it("returns empty result when fields array is empty", () => {
    const spec: BulkSpec = { ...emptyBulkSpec, find: "Alice", fields: [] };
    const result = computeBulk(commits, emptyEdits, spec);
    expect(result.matchedCommits).toBe(0);
  });

  it("performs literal find & replace on authorName", () => {
    const spec: BulkSpec = {
      ...emptyBulkSpec,
      fields: ["authorName"],
      find: "Alice",
      replace: "Alicia",
    };
    const result = computeBulk(commits, emptyEdits, spec);
    expect(result.matchedCommits).toBe(2);
    expect(result.replacements).toBe(2);
    expect(result.edits["aaa"]?.authorName).toBe("Alicia");
    expect(result.edits["ccc"]?.authorName).toBe("Alicia");
    expect(result.edits["bbb"]).toBeUndefined();
    expect(result.error).toBeNull();
  });

  it("performs literal find & replace on message field", () => {
    const spec: BulkSpec = {
      ...emptyBulkSpec,
      fields: ["message"],
      find: "bug",
      replace: "issue",
    };
    const result = computeBulk(commits, emptyEdits, spec);
    expect(result.matchedCommits).toBe(1);
    expect(result.edits["aaa"]?.message).toBe("Fix issue in parser");
  });

  it("supports case-insensitive mode", () => {
    const spec: BulkSpec = {
      ...emptyBulkSpec,
      fields: ["authorName"],
      find: "alice",
      replace: "ALICE",
      caseSensitive: false,
    };
    const result = computeBulk(commits, emptyEdits, spec);
    expect(result.matchedCommits).toBe(2);
    expect(result.edits["aaa"]?.authorName).toBe("ALICE");
  });

  it("supports regex mode", () => {
    const spec: BulkSpec = {
      ...emptyBulkSpec,
      fields: ["authorEmail"],
      find: "@.*\\.com$",
      replace: "@newdomain.org",
      regex: true,
    };
    const result = computeBulk(commits, emptyEdits, spec);
    expect(result.matchedCommits).toBe(3);
    expect(result.edits["aaa"]?.authorEmail).toBe("alice@newdomain.org");
    expect(result.edits["bbb"]?.authorEmail).toBe("bob@newdomain.org");
  });

  it("returns error for invalid regex", () => {
    const spec: BulkSpec = {
      ...emptyBulkSpec,
      fields: ["message"],
      find: "[invalid",
      replace: "",
      regex: true,
    };
    const result = computeBulk(commits, emptyEdits, spec);
    expect(result.error).not.toBeNull();
    expect(result.matchedCommits).toBe(0);
  });

  it("rejects ReDoS patterns via checkRegexSafety", () => {
    const spec: BulkSpec = {
      ...emptyBulkSpec,
      fields: ["message"],
      find: "(a+)+$",
      replace: "",
      regex: true,
    };
    const result = computeBulk(commits, emptyEdits, spec);
    expect(result.error).toContain("nested quantifiers");
    expect(result.matchedCommits).toBe(0);
  });

  it("composes with pending edits", () => {
    const pendingEdits: Record<string, CommitEdit> = {
      aaa: { hash: "aaa", authorName: "Alice Smith" },
    };
    const spec: BulkSpec = {
      ...emptyBulkSpec,
      fields: ["authorName"],
      find: "Alice Smith",
      replace: "A. Smith",
    };
    const result = computeBulk(commits, pendingEdits, spec);
    // Should find "Alice Smith" in the pending edit
    expect(result.matchedCommits).toBe(1);
    expect(result.edits["aaa"]?.authorName).toBe("A. Smith");
  });

  it("searches multiple fields at once", () => {
    const spec: BulkSpec = {
      ...emptyBulkSpec,
      fields: ["authorName", "authorEmail"],
      find: "alice",
      replace: "ALICE",
      caseSensitive: false,
    };
    const result = computeBulk(commits, emptyEdits, spec);
    // aaa: authorName matches, authorEmail matches
    // ccc: authorName matches, authorEmail matches
    expect(result.matchedCommits).toBe(2);
    expect(result.edits["aaa"]?.authorName).toBe("ALICE");
    expect(result.edits["aaa"]?.authorEmail).toBe("ALICE@example.com");
  });

  it("keeps $ literal in replacement when not in regex mode", () => {
    const spec: BulkSpec = {
      ...emptyBulkSpec,
      fields: ["authorName"],
      find: "Alice",
      replace: "$Alice$",
    };
    const result = computeBulk(commits, emptyEdits, spec);
    expect(result.edits["aaa"]?.authorName).toBe("$Alice$");
  });
});
