/** Read one complete base-10 integer experiment setting from an environment object. */
export function integerFromEnvironment(environment, name, fallback, minimum) {
  const raw = environment[name];
  if (raw === undefined) return fallback;
  if (!/^[+-]?\d+$/u.test(raw)) {
    throw new Error(`${name} must be an integer greater than or equal to ${minimum}`);
  }
  const parsed = Number(raw);
  if (!Number.isSafeInteger(parsed) || parsed < minimum) {
    throw new Error(`${name} must be an integer greater than or equal to ${minimum}`);
  }
  return parsed;
}
