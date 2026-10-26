// Extracts a named example from a Rust test file imported with `?raw`.
// A snippet starts at a line containing `// @example-start: <name>` and ends
// at the next line containing `// @example-end`, both exclusive.
export function snippet(source: string, name: string): string {
  const lines = source.split('\n');
  const start = lines.findIndex((line) => line.includes(`// @example-start: ${name}`));
  if (start < 0) throw new Error(`snippet "${name}" not found`);
  const end = lines.findIndex((line, index) => index > start && line.includes('// @example-end'));
  if (end < 0) throw new Error(`snippet "${name}" has no end marker`);
  return lines.slice(start + 1, end).join('\n');
}
