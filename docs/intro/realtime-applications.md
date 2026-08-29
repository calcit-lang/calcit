---
title: "Real-time Application Model"
scope: "core"
kind: "guide"
category: "intro"
aliases:
  - "calcium workflow"
  - "realtime web"
  - "websocket diff patch"
  - "incremental synchronization"
---
# Real-time Application Model

Calcit's primary web-application model is a stateful real-time system rather than a collection of unrelated framework conventions. [Calcium Workflow](https://github.com/Cumulo/calcium-workflow) is the reference template for this design.

## One deterministic state transition path

The server receives a typed operation and applies it through one serial updater. Business state transitions stay synchronous and deterministic. Timers, WebSocket transport, filesystem access, HTTP, and other asynchronous capabilities deliver events back to that path instead of mutating application state concurrently.

## Typed protocol boundaries

Untrusted WebSocket EDN is decoded once into nominal client/server message enums. Revisions, operations, snapshots, patch batches, and recoverable decode failures remain typed after that boundary. Dynamic is appropriate at host and wire boundaries, not throughout updater or projection code.

## Projection before transport

The server derives a client-visible projection from authoritative state. Respo renders the browser view, while Recollect provides structural diff/patch operations for incremental synchronization. Projection functions and memoization must remain pure and deterministic so unchanged state does not produce artificial patches.

## Revision, acknowledgement, and resynchronization

Every snapshot and patch belongs to a monotonic revision. A patch states its base revision; the client applies it only when that base matches local state and acknowledges successful advancement. Missing, invalid, reordered, or oversized patches trigger a full snapshot rather than allowing silent divergence.

## Bounded asynchronous work

Ordinary dispatches coalesce briefly so rapid operations produce one incremental update. Backpressure retries use a separate, slower schedule. Native async FFI queues are bounded and cancellable, and transport outcomes are typed so accepted, backpressured, oversized, and closed sends cannot be confused.

## Observable convergence

Application metrics should cover diff latency, patch bytes, pending acknowledgements, resyncs, and slow-client state. Transport metrics should cover queue depth, queued bytes, age, retries, and disconnect causes. Fault-injection tests should verify skipped patches, reconnects, background recovery, and slow readers converge to the latest snapshot.

## Design consistency

When adding a language or ecosystem feature for this model:

- strengthen nominal types and traits before adding ad hoc syntax;
- expose reusable capabilities primarily as methods;
- preserve one deterministic business-state path;
- keep host effects and Dynamic values at documented boundaries;
- prefer revisioned, testable protocols over implicit timing assumptions;
- validate changes in Calcium Workflow and at least one real application such as TopixIM or Timegrass.

These principles also guide Calcit core, Respo, Recollect, `calcit-wss`, `ws-edn.calcit`, `cumulo-util.calcit`, and related native modules. Each project owns a specific layer, but the combined application model should remain coherent.
