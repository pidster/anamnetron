import { describe, it, expect } from "vitest";
import { parseHash, buildHash } from "../router";

describe("parseHash", () => {
  it("returns empty state for empty hash", () => {
    expect(parseHash("")).toEqual({});
  });

  it("returns empty state for hash with only #", () => {
    expect(parseHash("#")).toEqual({});
  });

  it("parses view and path", () => {
    expect(parseHash("#treemap=/aeon/wal")).toEqual({
      view: "treemap",
      path: "/aeon/wal",
    });
  });

  it("parses view with empty path", () => {
    expect(parseHash("#matrix=&v=1")).toEqual({
      view: "matrix",
      version: 1,
    });
  });

  it("parses view, path, and version", () => {
    expect(parseHash("#treemap=/aeon/wal&v=1")).toEqual({
      view: "treemap",
      path: "/aeon/wal",
      version: 1,
    });
  });

  it("parses diff parameter", () => {
    expect(parseHash("#treemap=&v=2&diff=1")).toEqual({
      view: "treemap",
      version: 2,
      diff: 1,
    });
  });

  it("parses mermaid parameter", () => {
    expect(parseHash("#mermaid=&v=1&mermaid=flowchart")).toEqual({
      view: "mermaid",
      version: 1,
      mermaid: "flowchart",
    });
  });

  it("parses view-only (no params)", () => {
    expect(parseHash("#treemap=")).toEqual({
      view: "treemap",
    });
  });

  it("decodes URI components in path", () => {
    expect(parseHash("#treemap=/aeon%20core")).toEqual({
      view: "treemap",
      path: "/aeon core",
    });
  });

  it("parses legacy format with version and node", () => {
    expect(parseHash("#v=1&node=/aeon/core")).toEqual({
      version: 1,
      path: "/aeon/core",
    });
  });

  it("parses legacy format with scope as path", () => {
    expect(parseHash("#v=1&scope=myNode")).toEqual({
      version: 1,
      path: "myNode",
    });
  });

  it("legacy scope takes priority over node", () => {
    expect(parseHash("#v=1&scope=root&node=child")).toEqual({
      version: 1,
      path: "root",
    });
  });

  it("parses legacy format with view param", () => {
    expect(parseHash("#v=1&view=bundle")).toEqual({
      version: 1,
      view: "bundle",
    });
  });
});

describe("buildHash", () => {
  it("returns empty string for empty state", () => {
    expect(buildHash({})).toBe("");
  });

  it("builds hash with view only", () => {
    expect(buildHash({ view: "treemap" })).toBe("#treemap=");
  });

  it("builds hash with view and version", () => {
    expect(buildHash({ view: "treemap", version: 1 })).toBe("#treemap=&v=1");
  });

  it("builds hash with view and path", () => {
    expect(buildHash({ view: "matrix", path: "/aeon/core" })).toBe("#matrix=/aeon/core");
  });

  it("builds hash with view, path, and version", () => {
    expect(buildHash({ view: "treemap", path: "/aeon/wal", version: 1 })).toBe(
      "#treemap=/aeon/wal&v=1",
    );
  });

  it("preserves slashes in path", () => {
    const hash = buildHash({ view: "treemap", path: "/aeon/wal/log" });
    expect(hash).toBe("#treemap=/aeon/wal/log");
  });

  it("includes diff parameter", () => {
    const hash = buildHash({ view: "treemap", version: 2, diff: 1 });
    expect(hash).toContain("diff=1");
  });

  it("omits undefined values", () => {
    const hash = buildHash({ view: "treemap", version: 1, path: undefined });
    expect(hash).toBe("#treemap=&v=1");
  });

  it("round-trips view and path", () => {
    const state = { view: "treemap", path: "/aeon/wal", version: 3 };
    expect(parseHash(buildHash(state))).toEqual(state);
  });

  it("round-trips empty path", () => {
    const state = { view: "matrix", version: 1 };
    expect(parseHash(buildHash(state))).toEqual(state);
  });

  it("round-trips diff parameter", () => {
    const state = { view: "treemap", version: 3, diff: 1 };
    expect(parseHash(buildHash(state))).toEqual(state);
  });

  it("round-trips mermaid parameter", () => {
    const state = { view: "mermaid", version: 1, mermaid: "flowchart" };
    expect(parseHash(buildHash(state))).toEqual(state);
  });

  it("round-trips deep path", () => {
    const state = { view: "bundle", path: "/aeon/rest/handlers/auth", version: 2 };
    expect(parseHash(buildHash(state))).toEqual(state);
  });
});
