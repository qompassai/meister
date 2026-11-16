<!-- skills/README.md — repo skills catalog for meister -->

# skills/

Agent skills for working on meister, in the standard `SKILL.md`
format ([Agent Skills specification](https://agentskills.io/specification)).
A skill is a directory holding instructions an agent loads on
demand — procedural knowledge with a trigger, not config. Each
skill here is one directory with a `SKILL.md`: YAML frontmatter
(the catalog layer: `name` + `description`, which is the trigger)
followed by the operational core.

| Skill | Fires when |
|---|---|
| `meister-build-and-gate` | Building meister on primo, running its gates, validating a change, or regenerating the SCIP index. |
| `meister-lane-law` | Touching records, envelopes, intake, classification, or anything that could move content between lanes. |
| `meister-tool-surface` | Adding, changing, or promoting a tool (seam → real) on meister's MCP surface. |
| `meister-genre-packs` | Working on genre categories or packs: the thirteen categories, pack structure, worked examples, the teaching layer. |

These skills are repo-local: they describe this server's
discipline, grounded in the plan of record (../README.md). The
estate-wide Tiger Style skills (tiger-style-rust,
tiger-style-mcp) still govern the code and the surface; these
four carry the meister-specific application of them.
