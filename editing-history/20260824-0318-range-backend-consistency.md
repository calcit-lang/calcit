# range backend consistency and allocation

- Native and JavaScript `range` now reject non-finite inputs and use exact
  equality for the empty half-open interval, avoiding divergent edge behavior.
- JavaScript range generation handles negative steps and fills one backing
  array directly instead of creating a new `CalcitSliceList` wrapper for every
  item. A local 20 × 200,000-item benchmark improved from 40.96 ms to 20.71 ms.
- Native range generation reserves the estimated vector capacity with
  `try_reserve_exact`, retaining the existing iterative floating-point behavior
  while avoiding repeated vector growth and reporting impossible allocations.
- Regression coverage runs descending and fractional ranges in the native and
  JavaScript project suites; definition-attached tests also cover descending
  ranges in `calcit.core`.
