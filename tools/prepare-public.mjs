/** Stage only public web assets; the API server and monitoring sources stay private to the build. */
import { cp, mkdir, rm } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = fileURLToPath(new URL("../", import.meta.url));
const destination = path.join(root, "public");
await rm(destination, { recursive: true, force: true });
await mkdir(path.join(destination, "api"), { recursive: true });
for (const name of ["assets", "config.js", "data", "CNAME", ".nojekyll"]) {
  await cp(path.join(root, name), path.join(destination, name), {
    recursive: true,
  });
}
await cp(
  path.join(root, "api/openapi.json"),
  path.join(destination, "api/openapi.json"),
);
