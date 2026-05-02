// Minimal `clsx`-style class joiner. Falsy values (undefined, null,
// false, "") are dropped; truthy strings are concatenated with a single
// space. Avoids pulling in a runtime dependency for this trivial need.
type Cn = string | number | false | null | undefined;
export function cn(...parts: Cn[]): string {
  let out = "";
  for (const p of parts) {
    if (!p && p !== 0) continue;
    if (out) out += " ";
    out += p;
  }
  return out;
}
