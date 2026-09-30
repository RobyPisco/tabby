import { describe, expect, it } from "vitest";
import { isNewer } from "./updates";

describe("isNewer", () => {
  it("confronta numero per numero", () => {
    expect(isNewer("1.1.0", "1.0.0")).toBe(true);
    expect(isNewer("1.10.0", "1.9.0")).toBe(true);
    expect(isNewer("v2.0", "1.9.9")).toBe(true);
  });
  it("non segnala versioni uguali o più vecchie", () => {
    expect(isNewer("1.1.0", "1.1.0")).toBe(false);
    expect(isNewer("1.0.9", "1.1.0")).toBe(false);
  });
});
