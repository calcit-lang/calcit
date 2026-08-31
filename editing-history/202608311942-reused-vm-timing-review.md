# Reused VM timing review

- Verified the review finding against the cache-profile measurement path.
- Kept only `run_values` calls inside the reused-VM execution timer.
- Moved Calx result decoding after the timer so fresh, reused, and ordinary hot execution measurements use the same boundary.
