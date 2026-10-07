// Internal protocol shared by the runner and its host-level regression tests.
export const parseBackendSelection = (value, supported) => {
  const backends = value.split(",").filter(Boolean);
  if (backends.length === 0) throw new Error("--backend requires at least one backend");
  for (const backend of backends) {
    if (!supported.includes(backend)) throw new Error(`unknown backend: ${backend}`);
  }
  if (new Set(backends).size !== backends.length) throw new Error("--backend must not contain duplicate backends");
  return backends;
};

export const parseMarkers = (stdout) => {
  const traces = new Map();
  const events = [];
  let current;
  let last;
  for (const line of stdout.split(/\r?\n/)) {
    const marker = /^@@core-test(-end)?:(\d+)$/.exec(line);
    if (marker) {
      const index = Number(marker[2]);
      const kind = marker[1] ? "end" : "start";
      events.push([kind, index]);
      if (kind === "start") {
        current = last = index;
        traces.set(index, []);
      } else {
        current = undefined;
      }
    } else if (current !== undefined && line !== "") {
      traces.get(current).push(line);
    }
  }
  return { traces, last, events };
};

// A failed execution may stop on a prefix; a successful one must finish every test.
export const markerProtocolError = (events, tests, success) => {
  const expected = tests.flatMap((test) => [["start", test.index], ["end", test.index]]);
  for (let i = 0; i < events.length; i++) {
    if (JSON.stringify(events[i]) !== JSON.stringify(expected[i])) {
      return `invalid test marker at ${i}: expected ${JSON.stringify(expected[i])}, got ${JSON.stringify(events[i])}`;
    }
  }
  if (success && events.length !== expected.length) {
    return `successful process exited before all tests completed: received ${events.length}/${expected.length} test markers`;
  }
  return undefined;
};

export const hasNativeParity = (outcome, native, backend) => outcome?.status === "pass"
  && native?.status === "pass"
  && (backend === "native" || JSON.stringify(outcome.trace) === JSON.stringify(native.trace));
