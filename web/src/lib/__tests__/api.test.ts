// web/src/lib/__tests__/api.test.ts
import { describe, it, expect, vi, beforeEach } from "vitest";
import { getSnapshots, getGraph, getHealth, searchNodes, getDiff } from "../api";

// Mock fetch globally
const mockFetch = vi.fn();
vi.stubGlobal("fetch", mockFetch);

beforeEach(() => {
  mockFetch.mockReset();
});

function mockResponse(data: unknown, ok = true, status = 200) {
  return {
    ok,
    status,
    statusText: "OK",
    json: () => Promise.resolve(data),
  };
}

describe("api", () => {
  it("getHealth fetches /api/health", async () => {
    mockFetch.mockResolvedValueOnce(mockResponse({ status: "ok" }));
    const result = await getHealth();
    expect(result).toEqual({ status: "ok" });
    expect(mockFetch).toHaveBeenCalledWith("/api/health");
  });

  it("getSnapshots fetches /api/snapshots", async () => {
    const data = [{ version: 1, kind: "design", commit_ref: null }];
    mockFetch.mockResolvedValueOnce(mockResponse(data));
    const result = await getSnapshots();
    expect(result).toEqual(data);
    expect(mockFetch).toHaveBeenCalledWith("/api/snapshots");
  });

  it("getGraph fetches /api/snapshots/{v}/graph", async () => {
    const data = { elements: { nodes: [], edges: [] } };
    mockFetch.mockResolvedValueOnce(mockResponse(data));
    const result = await getGraph(1);
    expect(result).toEqual(data);
    expect(mockFetch).toHaveBeenCalledWith("/api/snapshots/1/graph");
  });

  it("searchNodes encodes path parameter", async () => {
    mockFetch.mockResolvedValueOnce(mockResponse([]));
    await searchNodes("/svt/**", 1);
    expect(mockFetch).toHaveBeenCalledWith("/api/search?path=%2Fsvt%2F**&version=1");
  });

  it("getDiff fetches correct endpoint", async () => {
    const data = {
      from_version: 1,
      to_version: 2,
      node_changes: [],
      edge_changes: [],
      summary: { nodes_added: 0, nodes_removed: 0, nodes_changed: 0, edges_added: 0, edges_removed: 0 },
    };
    mockFetch.mockResolvedValueOnce(mockResponse(data));
    const result = await getDiff(1, 2);
    expect(result).toEqual(data);
    expect(mockFetch).toHaveBeenCalledWith("/api/diff?from=1&to=2");
  });

  it("throws on HTTP error with server message", async () => {
    mockFetch.mockResolvedValueOnce(
      mockResponse({ error: "not found" }, false, 404),
    );
    await expect(getHealth()).rejects.toThrow("not found");
  });
});
