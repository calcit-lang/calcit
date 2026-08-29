---
title: "Overview"
scope: "core"
kind: "guide"
category: "intro"
aliases:
  - "overview"
  - "immutable data"
  - "pattern matching"
---
# Overview

## Immutable values and deterministic state

Calcit uses persistent immutable collections in both the Rust interpreter and JavaScript output. Application state changes are expressed as explicit operations passed through a deterministic updater. This keeps replay, diffing, hot reload, and synchronization understandable.

## Nominal domain modeling

Structs and enums describe application data and protocol envelopes. Pattern matching validates enum variants, while Option and Result make missing values and recoverable failures explicit. Static analysis preserves these relationships through function calls and collection operations.

## Traits and method-oriented capabilities

Traits describe capabilities independently from concrete data representation. Public APIs generally expose those capabilities as methods, with explicit trait calls available when dispatch needs disambiguation. This is the preferred extension model for collections, typed host objects, and reusable application abstractions.

## Code as data with Cirru syntax

Calcit macros transform syntax trees. Source is stored canonically in `calcit.cirru` and written with indentation, `$`, and local parentheses rather than delimiter-heavy collection syntax. CLI tree operations can inspect and update this source structurally.

## Native and JavaScript execution

The Rust interpreter supports native scripts and typed C FFI modules. JavaScript codegen emits ES Modules for browser and Node.js ecosystems. Both paths aim to preserve the same Calcit semantics while keeping host-specific effects at explicit boundaries.

## Interactive real-time applications

Calcit hot reload is designed to preserve running application state. In Calcium-style applications, a serial server updater produces typed client projections, Recollect computes structural differences, and revision/ack/resync messages keep browser state convergent over WebSocket. See [Real-time Application Model](realtime-applications.md).
