//! The `stream.channel` rendezvous -- version-1 frozen framing.
//!
//! # Direction
//!
//! The client only ever *sets attributes* (client → renderer), which keeps
//! ɴsɪ's dataflow unidirectional. Everything flowing the other way is
//! initiated by the **driver**: it connects to the endpoint named by
//! `stream.channel`, then sends the exported handles and one message per
//! publication (`data-model.md`, "Direction"). The client listens; it never
//! sends.
//!
//! ```text
//! client: bind(stream.channel), listen, accept ── driver: connect
//!                                              ←─ Hello   { version }
//!                                              ←─ Open    { extent, layers, ring, transport } + fd
//!                                              ←─ Publish { slot, serial, generation, timeline, extent }
//!                                              ←─ Resize  { extent } + fd
//!                                              ←─ Close   { final timeline value, dropped }
//! ```
//!
//! # Wire Format (frozen at `stream.version` 1)
//!
//! Every message is one frame. All integers are little-endian:
//!
//! ```text
//! u32 length   number of bytes that follow (tag + payload)
//! u8  tag      message discriminant
//! ... payload
//! ```
//!
//! | Tag | Message | Payload | Ancillary |
//! | --- | --- | --- | --- |
//! | `0x01` | `Hello` | `u32` version | -- |
//! | `0x02` | `Open` | `u32` width, `u32` height, `u32` ring, `u8` transport, `u32` layer count, then per layer: `u32` format, `u32` channels, `u32`+bytes name, `u32`+bytes `variablename` | segment fd via `SCM_RIGHTS` |
//! | `0x03` | `Publish` | `u64` slot, `u64` frame serial, `u64` scene generation, `u64` timeline value, `u32` width, `u32` height | -- |
//! | `0x04` | `Resize` | `u32` width, `u32` height | new segment fd via `SCM_RIGHTS` |
//! | `0x05` | `Close` | `u64` final timeline value, `u64` dropped | -- |
//!
//! Handles cross the boundary as descriptors passed with `SCM_RIGHTS`,
//! never as addresses (R2). A descriptor is attached to the first byte of
//! its frame, so it arrives with the first `recvmsg` that consumes any of
//! that frame.
//!
//! # Compatibility And Versioning
//!
//! - Tags, field order and field widths are frozen. Any change bumps
//!   `stream.version`.
//! - `Hello` is always the first frame and carries the vocabulary version
//!   the driver speaks. A client that does not implement it must fail
//!   loudly.
//! - **Unknown tags are rejected loudly** ([`Error::MalformedAttribute`]
//!   naming `stream.channel`); a client must never skip a frame it does not
//!   understand, because frames carry descriptors whose ownership it would
//!   leak (`data-model.md`, "Persistence And Migration").
//! - Frames larger than [`MAX_FRAME_BYTES`] are rejected rather than
//!   allocated.
//!
//! # Client Loss
//!
//! A failing send (`EPIPE`, or a short/closed socket) means the client is
//! gone. The driver then honors `stream.onclientloss`:
//!
//! - [`ClientLoss::Continue`] -- the publication is dropped,
//!   [`DriverChannel::dropped`] increments, [`DriverChannel::client_lost`]
//!   becomes true and the call returns `Ok(())`. The render continues.
//! - [`ClientLoss::Stop`] -- [`DriverChannel::should_stop`] becomes true and
//!   the call returns [`Error::ChannelClosed`]. The renderer polls the flag
//!   and stops.

use crate::{
    Error, Result,
    config::ClientLoss,
    layer::{Extent, Layer, LayerFormat},
    ring::Publication,
    transport::Transport,
};
use rustix::net::{
    RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, SendAncillaryBuffer,
    SendAncillaryMessage, SendFlags, recvmsg, sendmsg,
};
use std::{
    io::{IoSlice, IoSliceMut},
    mem::MaybeUninit,
    os::{
        fd::{AsFd, BorrowedFd, OwnedFd},
        unix::net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
};

/// Largest frame this implementation will read or write.
pub const MAX_FRAME_BYTES: usize = 1 << 20;

/// `Hello` message tag.
pub const TAG_HELLO: u8 = 0x01;
/// `Open` message tag.
pub const TAG_OPEN: u8 = 0x02;
/// `Publish` message tag.
pub const TAG_PUBLISH: u8 = 0x03;
/// `Resize` message tag.
pub const TAG_RESIZE: u8 = 0x04;
/// `Close` message tag.
pub const TAG_CLOSE: u8 = 0x05;

// ─── Messages ───────────────────────────────────────────────────────────────

/// A decoded channel message.
///
/// Not `Clone`: the descriptor-carrying variants own the descriptor they
/// received.
#[derive(Debug)]
pub enum Message {
    /// First frame: the vocabulary version the driver speaks.
    Hello {
        /// `stream.version` the driver implements.
        version: u32,
    },
    /// The stream opened; the segment descriptor is attached.
    Open {
        /// Extent of the first ring allocation.
        extent: Extent,
        /// Connected layers, in publication plane order.
        layers: Vec<Layer>,
        /// Number of ring slots.
        ring: usize,
        /// The negotiated transport.
        transport: Transport,
        /// The exported segment descriptor.
        fd: Option<OwnedFd>,
    },
    /// A publication was announced.
    Publish(Publication),
    /// The stream was reallocated; a new segment descriptor is attached.
    Resize {
        /// The new extent.
        extent: Extent,
        /// The new segment descriptor.
        fd: Option<OwnedFd>,
    },
    /// The stream closed.
    Close {
        /// Nothing will be signaled beyond this timeline value.
        final_timeline_value: u64,
        /// Publications dropped over the lifetime of the stream.
        dropped: u64,
    },
}

/// The `Open` message payload the driver sends.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OpenMessage {
    /// Extent of the first ring allocation.
    pub extent: Extent,
    /// Connected layers, in publication plane order.
    pub layers: Vec<Layer>,
    /// Number of ring slots.
    pub ring: usize,
    /// The negotiated transport.
    pub transport: Transport,
}

// ─── Client End ─────────────────────────────────────────────────────────────

/// The client end: binds `stream.channel` and waits for the driver.
#[derive(Debug)]
pub struct ClientChannel {
    listener: UnixListener,
    path: PathBuf,
}

impl ClientChannel {
    /// Bind the endpoint named by `stream.channel`.
    ///
    /// The socket file is removed when the [`ClientChannel`] is dropped.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when the path cannot be bound -- including when it
    /// already exists. A stale endpoint is reported, never silently
    /// replaced.
    pub fn bind(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        UnixListener::bind(&path)
            .map(|listener| Self {
                listener,
                path: path.clone(),
            })
            .map_err(|error| {
                Error::io(format!("bind of `{}`", path.display()), error)
            })
    }

    /// The bound path, i.e. the value of `stream.channel`.
    #[inline]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Wait for the driver to connect.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when `accept` fails.
    pub fn accept(&self) -> Result<ClientSession> {
        self.listener
            .accept()
            .map(|(stream, _)| ClientSession { stream })
            .map_err(|error| {
                Error::io("accept on the rendezvous socket", error)
            })
    }
}

impl Drop for ClientChannel {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// One accepted driver connection.
#[derive(Debug)]
pub struct ClientSession {
    stream: UnixStream,
}

impl ClientSession {
    /// Read the next message.
    ///
    /// # Errors
    ///
    /// - [`Error::ChannelClosed`] when the driver went away.
    /// - [`Error::MalformedAttribute`] naming `stream.channel` for an
    ///   unknown tag, a truncated payload or an oversized frame.
    /// - [`Error::Io`] for a failing `recvmsg`.
    pub fn recv(&mut self) -> Result<Message> {
        let mut length = [0u8; 4];
        let mut fd = None;

        recv_exact(self.stream.as_fd(), &mut length, &mut fd)?;

        let length = u32::from_le_bytes(length) as usize;

        if length == 0 || length > MAX_FRAME_BYTES {
            Err(framing(format!(
                "frame length {length} is out of range (1..={MAX_FRAME_BYTES})"
            )))?;
        }

        let mut frame = vec![0u8; length];
        recv_exact(self.stream.as_fd(), &mut frame, &mut fd)?;

        decode(&frame, fd)
    }
}

// ─── Driver End ─────────────────────────────────────────────────────────────

/// The driver end: connects to `stream.channel` and sends every
/// reverse-direction message.
#[derive(Debug)]
pub struct DriverChannel {
    stream: UnixStream,
    on_client_loss: ClientLoss,
    client_lost: bool,
    should_stop: bool,
    dropped: u64,
}

impl DriverChannel {
    /// Connect to the endpoint named by `stream.channel`.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when the endpoint cannot be reached.
    pub fn connect(
        path: impl AsRef<Path>,
        on_client_loss: ClientLoss,
    ) -> Result<Self> {
        let path = path.as_ref();

        UnixStream::connect(path)
            .map(|stream| Self {
                stream,
                on_client_loss,
                client_lost: false,
                should_stop: false,
                dropped: 0,
            })
            .map_err(|error| {
                Error::io(format!("connect to `{}`", path.display()), error)
            })
    }

    /// Whether a send has failed because the client vanished.
    #[inline]
    pub const fn client_lost(&self) -> bool {
        self.client_lost
    }

    /// Whether the renderer should stop (`stream.onclientloss "stop"` and
    /// the client is gone).
    #[inline]
    pub const fn should_stop(&self) -> bool {
        self.should_stop
    }

    /// Publications dropped because the client was gone.
    #[inline]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Send the `Hello` frame. Always the first frame.
    ///
    /// # Errors
    ///
    /// See [`DriverChannel::send_publish`].
    pub fn send_hello(&mut self, version: u32) -> Result<()> {
        let mut frame = vec![TAG_HELLO];
        frame.extend_from_slice(&version.to_le_bytes());

        self.send(&frame, None)
    }

    /// Send the `Open` frame with the exported segment descriptor.
    ///
    /// # Errors
    ///
    /// See [`DriverChannel::send_publish`].
    pub fn send_open(
        &mut self,
        open: &OpenMessage,
        fd: Option<BorrowedFd<'_>>,
    ) -> Result<()> {
        let mut frame = vec![TAG_OPEN];
        frame.extend_from_slice(&open.extent.width.to_le_bytes());
        frame.extend_from_slice(&open.extent.height.to_le_bytes());
        frame.extend_from_slice(&(open.ring as u32).to_le_bytes());
        frame.push(open.transport.as_wire());
        frame.extend_from_slice(&(open.layers.len() as u32).to_le_bytes());

        open.layers.iter().for_each(|layer| {
            frame.extend_from_slice(&layer.format.as_wire().to_le_bytes());
            frame.extend_from_slice(&layer.channels.to_le_bytes());
            push_string(&mut frame, &layer.name);
            push_string(&mut frame, &layer.variable_name);
        });

        self.send(&frame, fd)
    }

    /// Send one `Publish` frame.
    ///
    /// # Errors
    ///
    /// [`Error::ChannelClosed`] when the client is gone *and*
    /// `stream.onclientloss` is `"stop"`; [`Error::Io`] for other send
    /// failures. Under `"continue"` a lost client is not an error -- see the
    /// module documentation.
    pub fn send_publish(&mut self, publication: &Publication) -> Result<()> {
        let mut frame = vec![TAG_PUBLISH];
        frame
            .extend_from_slice(&(publication.image_index as u64).to_le_bytes());
        frame.extend_from_slice(&publication.frame_serial.to_le_bytes());
        frame.extend_from_slice(&publication.scene_generation.to_le_bytes());
        frame.extend_from_slice(&publication.timeline_value.to_le_bytes());
        frame.extend_from_slice(&publication.extent.width.to_le_bytes());
        frame.extend_from_slice(&publication.extent.height.to_le_bytes());

        self.send(&frame, None)
    }

    /// Send a `Resize` frame with the new segment descriptor.
    ///
    /// # Errors
    ///
    /// See [`DriverChannel::send_publish`].
    pub fn send_resize(
        &mut self,
        extent: Extent,
        fd: Option<BorrowedFd<'_>>,
    ) -> Result<()> {
        let mut frame = vec![TAG_RESIZE];
        frame.extend_from_slice(&extent.width.to_le_bytes());
        frame.extend_from_slice(&extent.height.to_le_bytes());

        self.send(&frame, fd)
    }

    /// Send the final `Close` frame.
    ///
    /// # Errors
    ///
    /// See [`DriverChannel::send_publish`].
    pub fn send_close(
        &mut self,
        final_timeline_value: u64,
        dropped: u64,
    ) -> Result<()> {
        let mut frame = vec![TAG_CLOSE];
        frame.extend_from_slice(&final_timeline_value.to_le_bytes());
        frame.extend_from_slice(&dropped.to_le_bytes());

        self.send(&frame, None)
    }

    /// Frame, send, and apply the `stream.onclientloss` policy.
    fn send(&mut self, frame: &[u8], fd: Option<BorrowedFd<'_>>) -> Result<()> {
        if self.client_lost {
            return self.lost();
        }

        let mut buffer = Vec::with_capacity(4 + frame.len());
        buffer.extend_from_slice(&(frame.len() as u32).to_le_bytes());
        buffer.extend_from_slice(frame);

        match send_all(self.stream.as_fd(), &buffer, fd) {
            Ok(()) => Ok(()),
            Err(Error::ChannelClosed) => {
                self.client_lost = true;
                self.lost()
            }
            Err(error) => Err(error),
        }
    }

    /// Honor `stream.onclientloss`.
    fn lost(&mut self) -> Result<()> {
        match self.on_client_loss {
            ClientLoss::Continue => {
                self.dropped += 1;
                Ok(())
            }
            ClientLoss::Stop => {
                self.should_stop = true;
                Err(Error::ChannelClosed)
            }
        }
    }
}

// ─── Framing ────────────────────────────────────────────────────────────────

fn framing(reason: impl Into<String>) -> Error {
    Error::malformed("stream.channel", reason)
}

fn push_string(frame: &mut Vec<u8>, value: &str) {
    frame.extend_from_slice(&(value.len() as u32).to_le_bytes());
    frame.extend_from_slice(value.as_bytes());
}

/// Send `buffer` in full, attaching `fd` to the first `sendmsg`.
fn send_all(
    socket: BorrowedFd<'_>,
    buffer: &[u8],
    fd: Option<BorrowedFd<'_>>,
) -> Result<()> {
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    let attached = fd.map(|fd| [fd]);

    if let Some(fds) = attached.as_ref() {
        ancillary.push(SendAncillaryMessage::ScmRights(fds));
    }

    let mut sent = 0;

    while sent < buffer.len() {
        let written = sendmsg(
            socket,
            &[IoSlice::new(&buffer[sent..])],
            &mut ancillary,
            // NOSIGNAL turns a vanished client into an `EPIPE` return
            // instead of a process-wide signal.
            SendFlags::NOSIGNAL,
        )
        .map_err(|error| match error {
            rustix::io::Errno::PIPE | rustix::io::Errno::CONNRESET => {
                Error::ChannelClosed
            }
            other => Error::io("sendmsg on the rendezvous socket", other),
        })?;

        if written == 0 {
            Err(Error::ChannelClosed)?;
        }

        sent += written;
        ancillary.clear();
    }

    Ok(())
}

/// Fill `buffer`, capturing a descriptor if one arrives with any chunk.
fn recv_exact(
    socket: BorrowedFd<'_>,
    buffer: &mut [u8],
    fd: &mut Option<OwnedFd>,
) -> Result<()> {
    let mut read = 0;

    while read < buffer.len() {
        let mut space =
            [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);

        let received = recvmsg(
            socket,
            &mut [IoSliceMut::new(&mut buffer[read..])],
            &mut ancillary,
            RecvFlags::empty(),
        )
        .map_err(|error| match error {
            rustix::io::Errno::PIPE | rustix::io::Errno::CONNRESET => {
                Error::ChannelClosed
            }
            other => Error::io("recvmsg on the rendezvous socket", other),
        })?;

        ancillary.drain().for_each(|message| {
            if let RecvAncillaryMessage::ScmRights(rights) = message {
                rights.for_each(|received| {
                    // The framing attaches at most one descriptor per
                    // frame; anything beyond that is closed on drop.
                    if fd.is_none() {
                        *fd = Some(received);
                    }
                });
            }
        });

        if received.bytes == 0 {
            Err(Error::ChannelClosed)?;
        }

        read += received.bytes;
    }

    Ok(())
}

/// Decode one frame.
fn decode(frame: &[u8], fd: Option<OwnedFd>) -> Result<Message> {
    let mut reader = Reader { frame, cursor: 1 };

    match frame[0] {
        TAG_HELLO => Ok(Message::Hello {
            version: reader.u32()?,
        }),
        TAG_OPEN => {
            let extent = Extent::new(reader.u32()?, reader.u32()?);
            let ring = reader.u32()? as usize;
            let transport =
                Transport::from_wire(reader.u8()?).ok_or_else(|| {
                    framing("unknown transport discriminant in `Open`")
                })?;
            let count = reader.u32()? as usize;

            let layers = (0..count)
                .map(|_| {
                    let wire = reader.u32()?;
                    let format =
                        LayerFormat::from_wire(wire).ok_or_else(|| {
                            framing(format!(
                                "unknown pixel format {wire} in `Open`"
                            ))
                        })?;
                    let channels = reader.u32()?;
                    let name = reader.string()?;
                    let variable_name = reader.string()?;

                    Ok(Layer::new(name, variable_name, format, channels))
                })
                .collect::<Result<Vec<_>>>()?;

            Ok(Message::Open {
                extent,
                layers,
                ring,
                transport,
                fd,
            })
        }
        TAG_PUBLISH => Ok(Message::Publish(Publication {
            image_index: reader.u64()? as usize,
            frame_serial: reader.u64()?,
            scene_generation: reader.u64()?,
            timeline_value: reader.u64()?,
            extent: Extent::new(reader.u32()?, reader.u32()?),
        })),
        TAG_RESIZE => Ok(Message::Resize {
            extent: Extent::new(reader.u32()?, reader.u32()?),
            fd,
        }),
        TAG_CLOSE => Ok(Message::Close {
            final_timeline_value: reader.u64()?,
            dropped: reader.u64()?,
        }),
        unknown => Err(framing(format!(
            "unknown message tag {unknown:#04x}; version-1 clients reject \
             frames they cannot account for"
        ))),
    }
}

struct Reader<'a> {
    frame: &'a [u8],
    cursor: usize,
}

impl Reader<'_> {
    fn take(&mut self, len: usize) -> Result<&[u8]> {
        let start = self.cursor;
        let end = start
            .checked_add(len)
            .filter(|end| *end <= self.frame.len())
            .ok_or_else(|| {
                framing(format!(
                    "frame is truncated: {len} more bytes needed at offset \
                     {start} of {}",
                    self.frame.len()
                ))
            })?;

        self.cursor = end;

        Ok(&self.frame[start..end])
    }

    fn u8(&mut self) -> Result<u8> {
        self.take(1).map(|bytes| bytes[0])
    }

    fn u32(&mut self) -> Result<u32> {
        self.take(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("4 bytes")))
    }

    fn u64(&mut self) -> Result<u64> {
        self.take(8)
            .map(|bytes| u64::from_le_bytes(bytes.try_into().expect("8 bytes")))
    }

    fn string(&mut self) -> Result<String> {
        let len = self.u32()? as usize;

        self.take(len).and_then(|bytes| {
            core::str::from_utf8(bytes)
                .map(str::to_string)
                .map_err(|error| {
                    framing(format!("string is not valid UTF-8: {error}"))
                })
        })
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_tag_is_rejected() {
        assert!(matches!(
            decode(&[0x7f, 0, 0, 0, 0], None),
            Err(Error::MalformedAttribute { .. })
        ));
    }

    #[test]
    fn truncated_frame_is_rejected() {
        assert!(matches!(
            decode(&[TAG_HELLO, 1, 0], None),
            Err(Error::MalformedAttribute { .. })
        ));
    }
}
