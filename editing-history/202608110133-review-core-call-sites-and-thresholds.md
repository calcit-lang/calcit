## Follow-up review fixes for nullable list-first

- Migrate remaining guarded Core reads in max/min, comparison folding, and list destructuring to `&list:nth ... 0`.
- Tag the empty-list regression as both `:core` and `:unit`.
- Make edit-advice fixtures assert exact 31/32-node boundary behavior, including one-sided threshold crossings.
