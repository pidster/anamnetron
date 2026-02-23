import { describe, it, expect } from "vitest";
import { parseHash, buildHash } from "../router";

describe("parseHash", () => {
  it("returns empty state for empty hash", () => {
    expect(parseHash("")).toEqual({});
  });

  it("returns empty state for hash with only #", () => {
    expect(parseHash("#")).toEqual({});
  });

  it("parses version", () => {
    expect(parseHash("#v=1")).toEqual({ version: 1 });
  });

  it("parses version and node", () => {
    expect(parseHash("#v=1&node=abc")).toEqual({ version: 1, node: "abc" });
  });

  it("parses layout", () => {
    expect(parseHash("#v=1&layout=dagre")).toEqual({ version: 1, layout: "dagre" });
  });

  it("parses all fields", () => {
    expect(parseHash("#v=2&node=n1&layout=cose-bilkent")).toEqual({
      version: 2,
      node: "n1",
      layout: "cose-bilkent",
    });
  });

  it("decodes URI components", () => {
    expect(parseHash("#v=1&node=%2Fsvt%2Fcore")).toEqual({ version: 1, node: "/svt/core" });
  });

  it("parses diff parameter", () => {
    expect(parseHash("#v=2&diff=1")).toEqual({ version: 2, diff: 1 });
  });

  it("parses scope parameter", () => {
    expect(parseHash("#v=1&scope=myNode")).toEqual({ version: 1, scope: "myNode" });
  });

  it("parses mermaid parameter", () => {
    expect(parseHash("#v=1&mermaid=flowchart")).toEqual({ version: 1, mermaid: "flowchart" });
  });

  it("parses scope and mermaid together", () => {
    expect(parseHash("#v=1&scope=root&mermaid=c4")).toEqual({
      version: 1,
      scope: "root",
      mermaid: "c4",
    });
  });
});

describe("buildHash", () => {
  it("returns empty string for empty state", () => {
    expect(buildHash({})).toBe("");
  });

  it("builds hash from version only", () => {
    expect(buildHash({ version: 1 })).toBe("#v=1");
  });

  it("includes node when present", () => {
    const hash = buildHash({ version: 1, node: "abc" });
    expect(hash).toContain("v=1");
    expect(hash).toContain("node=abc");
  });

  it("encodes special characters in node", () => {
    const hash = buildHash({ version: 1, node: "/svt/core" });
    expect(hash).toContain("node=%2Fsvt%2Fcore");
  });

  it("omits undefined values", () => {
    expect(buildHash({ version: 1, node: undefined })).toBe("#v=1");
  });

  it("round-trips through parseHash", () => {
    const state = { version: 3, node: "/svt/core", layout: "dagre" };
    expect(parseHash(buildHash(state))).toEqual(state);
  });

  it("includes diff when present", () => {
    const hash = buildHash({ version: 2, diff: 1 });
    expect(hash).toContain("v=2");
    expect(hash).toContain("diff=1");
  });

  it("round-trips diff parameter", () => {
    const state = { version: 3, diff: 1, layout: "dagre" };
    expect(parseHash(buildHash(state))).toEqual(state);
  });

  it("includes scope when present", () => {
    const hash = buildHash({ version: 1, scope: "myNode" });
    expect(hash).toContain("scope=myNode");
  });

  it("includes mermaid when present", () => {
    const hash = buildHash({ version: 1, mermaid: "flowchart" });
    expect(hash).toContain("mermaid=flowchart");
  });

  it("round-trips scope and mermaid", () => {
    const state = { version: 2, scope: "/svt/core", mermaid: "c4" };
    expect(parseHash(buildHash(state))).toEqual(state);
  });
});
