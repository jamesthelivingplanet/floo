/**
 * OS-level port probing.
 *
 * We try to bind a fresh socket synchronously. If bind succeeds, nothing else
 * holds the port at this instant. See SPEC.md - the bind check is a guard,
 * not a registration trigger; we never adopt a bound-but-untracked port.
 */
import { createServer } from "node:net";

export function isPortFreeOnOs(port: number, host = "127.0.0.1"): Promise<boolean> {
  return new Promise((res) => {
    const srv = createServer();
    srv.once("error", () => res(false));
    srv.listen(port, host, () => {
      srv.close(() => res(true));
    });
  });
}
