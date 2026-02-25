import { describe, it, expect } from "vitest";
import { parseHash, buildHash } from "../router";

describe("parseHash", () => {
  it("returns empty state for empty hash", () => {
    expect(parseHash("")).toEqual({});
  });

  it("returns empty state for hash with only #", () => {
    expect(parseHash("#")).toEqual({});
  });

  it("parses new format with view and focus path", () => {
    expect(parseHash("#treemap:/aeon/wal&v=1")).toEqual({
      view: "treemap",
      focusPath: "/aeon/wal",
      version: 1,
    });
  });

  it("parses new format with no focus path", () => {
    expect(parseHash("#matrix:&v=1")).toEqual({
      view: "matrix",
      version: 1,
    });
  });

  it("parses new format with node parameter", () => {
    expect(parseHash("#treemap:/aeon/wal&v=1&node=abc")).toEqual({
      view: "treemap",
      focusPath: "/aeon/wal",
      version: 1,
      node: "abc",
    });
  });

  it("parses new format with diff parameter", () => {
    expect(parseHash("#treemap:&v=2&diff=1")).toEqual({
      view: "treemap",
      version: 2,
      diff: 1,
    });
  });

  it("parses new format with mermaid parameter", () => {
    expect(parseHash("#mermaid:&v=1&mermaid=flowchart")).toEqual({
      view: "mermaid",
      version: 1,
      mermaid: "flowchart",
    });
  });

  it("parses view-only prefix (no params)", () => {
    expect(parseHash("#treemap:/aeon/wal")).toEqual({
      view: "treemap",
      focusPath: "/aeon/wal",
    });
  });

  it("parses legacy format (backwards compatibility)", () => {
    expect(parseHash("#v=1&node=abc")).toEqual({
      version: 1,
      node: "abc",
    });
  });

  it("parses legacy format with scope as focusPath", () => {
    expect(parseHash("#v=1&scope=myNode")).toEqual({
      version: 1,
      focusPath: "myNode",
    });
  });

  it("parses legacy format with view param", () => {
    expect(parseHash("#v=1&view=treemap")).toEqual({
      version: 1,
      view: "treemap",
    });
  });

  it("parses legacy format with all fields", () => {
    expect(parseHash("#v=2&node=n1&scope=root&mermaid=flowchart")).toEqual({
      version: 2,
      node: "n1",
      focusPath: "root",
      mermaid: "flowchart",
    });
  });

  it("decodes URI components in focus path", () => {
    expect(parseHash("#treemap:/svt%2Fcore&v=1")).toEqual({
      view: "treemap",
      focusPath: "/svt/core",
      version: 1,
    });
  });
});

describe("buildHash", () => {
  it("returns empty string for empty state", () => {
    expect(buildHash({})).toBe("");
  });

  it("builds hash with view and version", () => {
    const hash = buildHash({ view: "treemap", version: 1 });
    expect(hash).toBe("#treemap:&v=1");
  });

  it("builds hash with view and focus path", () => {
    const hash = buildHash({ view: "treemap", focusPath: "/aeon/wal", version: 1 });
    expect(hash).toBe("#treemap:/aeon/wal&v=1");
  });

  it("builds hash with node parameter", () => {
    const hash = buildHash({ view: "treemap", version: 1, node: "abc" });
    expect(hash).toContain("treemap:");
    expect(hash).toContain("v=1");
    expect(hash).toContain("node=abc");
  });

  it("builds hash with diff parameter", () => {
    const hash = buildHash({ view: "treemap", version: 2, diff: 1 });
    expect(hash).toContain("v=2");
    expect(hash).toContain("diff=1");
  });

  it("preserves slashes in focus path", () => {
    const hash = buildHash({ view: "treemap", focusPath: "/aeon/wal/log" });
    expect(hash).toContain("/aeon/wal/log");
  });

  it("omits undefined values", () => {
    const hash = buildHash({ view: "treemap", version: 1, node: undefined });
    expect(hash).not.toContain("node=");
  });

  it("round-trips through parseHash", () => {
    const state = { view: "treemap", focusPath: "/aeon/wal", version: 3, node: "n1", mermaid: "flowchart" };
    expect(parseHash(buildHash(state))).toEqual(state);
  });

  it("round-trips empty focus path", () => {
    const state = { view: "matrix", version: 1 };
    expect(parseHash(buildHash(state))).toEqual(state);
  });

  it("round-trips diff parameter", () => {
    const state = { view: "treemap", version: 3, diff: 1 };
    expect(parseHash(buildHash(state))).toEqual(state);
  });
});
