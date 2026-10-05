// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Line-framed OME input-method protocol and its reconnecting host client.
#![forbid(unsafe_code)]

pub mod client;
pub mod protocol;

pub use client::{ImeLink, ImeLinkEvent, ImeLinkState, ImeTransport, TcpImeTransport};
pub use protocol::{FocusInfo, GuestMessage, HostMessage, ProtocolError, TextKey};
