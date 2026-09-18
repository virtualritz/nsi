# Contract: Parsing A Stream In Parallel

| Behavior | Status | Source evidence | Test evidence | Notes |
| --- | --- | --- | --- | --- |
| The scene equals the sequential parser's | Covered | `parallel.rs` | `tests/parallel.rs`: `a_large_scene_is_the_same_scene` (2 000 meshes), and every test below that compares scenes | Canonical form forgives only the allowed reordering |
| Later `SetAttribute`s on a node win, as in the stream | Covered | stable sort by key | `the_last_set_attribute_on_a_node_wins_as_in_the_stream`. Falsified: an unstable sort reddens it | |
| `Connect`s into one attribute keep stream order | Covered | key = destination handle + attribute | `connections_into_one_attribute_keep_their_order` (300). Falsified: keying by the whole edge reddens it and two others | Why it matters: research D3, rendered |
| Time samples keep order | Covered | per-handle key | `time_samples_keep_their_order` | |
| `Delete` is a barrier | Covered | `Phase::Barrier` | `a_delete_is_a_barrier`. Falsified: making `Delete` a modify reddens it | |
| `DeleteAttribute`, `Disconnect` are barriers | Covered | `Phase::Barrier` | `a_delete_attribute_is_a_barrier`, `a_disconnect_is_a_barrier` | |
| Nothing crosses a `RenderControl` or `Evaluate` | Covered | `Phase::Barrier` | `nothing_crosses_a_render_control_or_an_evaluate`, on a call log. Falsified: making them modifies reddens it -- and only it | The scene-comparison test for these could not see order; renamed to what it checks |
| Single-error streams report the sequential error | Covered | `apply_one`, the fallback in `parse` | six `*_is_the_same_error` tests: leading junk, junk between statements, bad value, unterminated string, missing action, sink refusal | Multi-error streams: the earliest found, documented |
| Faster | Covered | -- | `tests/throughput.rs`, fresh build, three runs: null sink 166 → 312-322 MiB/s; into 3Delight 62-64 → 73-87 MiB/s | 16 threads, 20 000 meshes, 5.3 MiB |
| 3Delight takes calls concurrently | **Refuted** | -- | Probe, no parser: 20 000 `SetAttribute`s 11.8 ms on 1 thread, 21.4 ms on 16 | Research D4; documented on `parse_stream_parallel` |
| CI runs it | Covered | `rust.yml`: `--features ...,parallel --test parallel` | CI on push | |

## Open

- **The scan is half the time into a null sink.** A parallel scan --
  chunks at newlines, each scanned assuming it starts outside a string,
  re-scanned when that is wrong -- is the next step if more speed is
  wanted. Into 3Delight the renderer, not the scan, limits.
