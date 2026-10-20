// The committed types are what openapi-typescript makes of docs/openapi.json, so the package never drifts
// from the contract: `bun run generate` writes them again.

import { expect, test } from "bun:test";
import openapiTS, { COMMENT_HEADER, astToString } from "openapi-typescript";

test("the committed types are the generated ones", async () => {
  const document = new URL("../../../../docs/openapi.json", import.meta.url);
  const generated = COMMENT_HEADER + astToString(await openapiTS(document));
  const committed = await Bun.file(new URL("../generated/openapi.d.ts", import.meta.url)).text();

  expect(committed).toBe(generated);
});
