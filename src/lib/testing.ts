// A three-line test harness for the library modules: the project has no test
// runner, and Node runs these `.test.ts` files directly (`npm test`). A failing
// check throws with its name, which is all a CI log needs.

let passed = 0;
const failures: string[] = [];

/** Run one named check; a throw inside it is a failure, not a crash. */
export function check(name: string, body: () => void): void {
  try {
    body();
    passed++;
  } catch (e) {
    failures.push(`${name}: ${e instanceof Error ? e.message : String(e)}`);
  }
}

/** An asynchronous check; `await` it before `report()`. */
export async function checkAsync(name: string, body: () => Promise<void>): Promise<void> {
  try {
    await body();
    passed++;
  } catch (e) {
    failures.push(`${name}: ${e instanceof Error ? e.message : String(e)}`);
  }
}

/** Print the tally and throw if anything failed (a non-zero exit for Node). */
export function report(): void {
  for (const f of failures) console.error(`FAIL ${f}`);
  console.log(`${passed} passed, ${failures.length} failed`);
  if (failures.length > 0) throw new Error(`${failures.length} check(s) failed`);
}
