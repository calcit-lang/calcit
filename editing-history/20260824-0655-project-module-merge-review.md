# Complete project-module merge review coverage

- Avoid cloning a dependency snapshot when it has no namespaces owned by the project package.
- Extend the transitive self-dependency regression to cover project subnamespaces and preserve strict conflicts for unrelated namespaces.
