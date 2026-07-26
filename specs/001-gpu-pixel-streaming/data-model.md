# Data Model: GPU-Resident Pixel Streaming

## Entities

- **StreamConfig** -- decoded `stream.*` attribute set: version, transport,
  publish mode, ring size, rendezvous channel, device UUID. Owner:
  `nsi-stream` vocabulary parser.
- **Layer** -- one connected `outputlayer`: name, `variablename`, pixel
  format, channel count. Owner: driver.
- **PublicationImage** -- one ring entry: extent, per-layer images (or array
  layers), backing memory handle, last-write timeline value. Owner: driver.
- **Publication** -- an announcement: image index, frame serial
  (monotonic), scene generation (count of applied synchronizes), semaphore
  value to wait on, extent. Owner: driver; consumed by client.
- **AcquireToken** -- client lease on a PublicationImage; must be released.
  Owner: client.
- **Transport** -- `GpuShared` | `Shm` | `Callback`. Selected at open.

## Lifecycle State Machine

```text
Configured --open()--> Open --first publication--> Streaming
Streaming --resize edit + synchronize--> Resizing --all old released--> Streaming
Streaming|Resizing --close()--> Draining --final semaphore value signaled--> Closed
open() failure --> Failed (typed error, no partial state)
```

Rules:

- open() validates version, transport viability, formats, and handles before
  allocating; failure leaves nothing to clean up.
- Resizing allocates a new ring at the new extent; pre-resize images are
  reclaimed only on release.
- Draining stops new publications, waits for the final timeline value, then
  releases GPU objects. Client channel-close is treated as close().

## Wire Format: Attribute Vocabulary (version 1)

Set by the client on the `outputdriver` node; forwarded verbatim to the
driver per the ɴsɪ spec.

| Attribute | ɴsɪ type | Req. | Meaning |
| --- | --- | --- | --- |
| `drivername` | string | yes | `"nsi-stream"`. |
| `stream.version` | int | yes | Vocabulary version; unsupported ⇒ typed open error. |
| `stream.transport` | string | no | `"auto"` (default), `"gpu"`, `"shm"`, `"callback"`. |
| `stream.publish` | string | no | `"commit"` (default) or `"continuous"`. |
| `stream.ring` | int | no | Ring size, default 3, min 2. |
| `stream.channel` | string | no | Rendezvous endpoint name (local socket) for cross-process handle export and publication messages. |
| `stream.device.uuid` | string | no | Adapter UUID the client renders on; driver must match or fail/fall back per transport rules. |
| `stream.callback.open` | pointer | no | In-process only: open notification closure. |
| `stream.callback.publish` | pointer | no | In-process only: publication notification closure. |
| `stream.callback.close` | pointer | no | In-process only: close notification closure. |
| `stream.onclientloss` | string | no | `"continue"` (default) or `"stop"` -- renderer behavior when the client vanishes. |

Direction of the reverse channel: the client only ever *sets attributes*
(client → renderer, preserving unidirectional ɴsɪ dataflow); the driver
initiates the reverse flow itself by connecting to `stream.channel` (or
invoking the in-process closures), over which it sends: exported memory and
semaphore handles at open/resize, then one Publication message per publish.

Message framing on `stream.channel`, handle passing mechanics (e.g.
`SCM_RIGHTS` on Unix), and the shm layout for the `shm` transport are
version-1-frozen and documented with the implementation; any change bumps
`stream.version`.

## Ownership And Concurrency

- Driver owns all GPU objects and the semaphore; the client owns only
  leases (AcquireTokens) and its rendezvous endpoint.
- The driver never writes an image with an outstanding lease; if no image is
  free at publish time, the publication is dropped (latest-wins) and a drop
  counter increments.
- Bucket writers (renderer threads) synchronize among themselves before a
  publication's semaphore value is signaled; the client only ever waits on
  the timeline value from the Publication message.

## Persistence And Migration

No on-disk state. The wire contract is versioned by `stream.version`
(R7): the driver rejects unknown versions loudly; the client rejects
unknown message types loudly. No silent downgrade.
